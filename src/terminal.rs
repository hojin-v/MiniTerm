use crate::{pty, shell::ShellProfile};
use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    grid::Dimensions,
    sync::FairMutex,
    term::Term,
    vte::ansi::Processor,
};
use egui::Context;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const INPUT_BUDGET: usize = 1024 * 1024;
const READ_CHUNK: usize = 16 * 1024;
const REPLY_BUDGET: usize = 64 * 1024;
const REPLY_BURST: f64 = 256.0 * 1024.0;
const REPLY_PER_SECOND: f64 = 128.0 * 1024.0;

struct ReplyRate {
    available: f64,
    updated: Instant,
}
impl Default for ReplyRate {
    fn default() -> Self {
        Self {
            available: REPLY_BURST,
            updated: Instant::now(),
        }
    }
}
impl ReplyRate {
    fn admit(&mut self, bytes: usize, now: Instant) -> bool {
        self.available = (self.available
            + now.saturating_duration_since(self.updated).as_secs_f64() * REPLY_PER_SECOND)
            .min(REPLY_BURST);
        self.updated = now;
        if bytes as f64 > self.available {
            false
        } else {
            self.available -= bytes as f64;
            true
        }
    }
}

enum PtyCommand {
    Write(Vec<u8>),
    Resize(u16, u16),
    Shutdown,
}

#[derive(Clone)]
pub struct Input {
    sender: SyncSender<PtyCommand>,
    queued: Arc<AtomicUsize>,
    win32: Arc<AtomicBool>,
    workers: Arc<crate::io_workers::Workers>,
    closed: Arc<AtomicBool>,
}
impl Input {
    pub fn win32_mode(&self) -> bool {
        self.win32.load(Ordering::Acquire)
    }
    pub fn send(&self, data: Vec<u8>) -> Result<(), String> {
        self.enqueue(data, false)
    }

    fn enqueue(&self, data: Vec<u8>, reply: bool) -> Result<(), String> {
        let count = data.len();
        let limit = INPUT_BUDGET + if reply { REPLY_BUDGET } else { 0 };
        if self.closed.load(Ordering::Acquire) || self.workers.stopped() {
            return Err("터미널 입력 연결이 닫혔습니다.".into());
        }
        if count > limit {
            return Err("붙여넣기는 1MB 이하로 나누어 주세요.".into());
        }
        self.queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                (queued.saturating_add(count) <= limit).then_some(queued.saturating_add(count))
            })
            .map_err(|_| "입력 큐가 가득 찼습니다.")?;
        let command = PtyCommand::Write(data);
        let sent = if reply {
            // Only the parser uses blocking backpressure, after releasing the VT lock.
            self.sender.send(command).is_ok()
        } else {
            self.sender.try_send(command).is_ok()
        };
        if !sent {
            self.release(count);
            return Err("입력 연결이 닫혔거나 큐가 가득 찼습니다.".into());
        }
        Ok(())
    }

    fn release(&self, count: usize) {
        let _ = self
            .queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                Some(n.saturating_sub(count))
            });
    }
}

struct Transport {
    input: Input,
    console: crate::console_guard::ConsoleHost,
    job: Arc<crate::native::ProcessJob>,
    error: Arc<Mutex<Option<String>>>,
    ctx: Context,
}
impl Transport {
    fn finish_input(&self) {
        self.input.closed.store(true, Ordering::Release);
        self.job.terminate();
        let _ = self.input.sender.try_send(PtyCommand::Shutdown);
    }
    fn stop(&self, error: Option<String>) {
        if !self.input.workers.stop() {
            return;
        }
        self.input.closed.store(true, Ordering::Release);
        if let Some(error) = error {
            *self.error.lock().unwrap_or_else(|p| p.into_inner()) = Some(error);
        }
        self.input.queued.store(0, Ordering::Release);
        self.job.terminate();
        self.console.terminate();
        // Wake an idle writer; a full queue also wakes it and the stop flag wins.
        let _ = self.input.sender.try_send(PtyCommand::Shutdown);
        self.ctx.request_repaint();
    }
}

type ColorFormatter = Arc<dyn Fn(alacritty_terminal::vte::ansi::Rgb) -> String + Sync + Send>;
type ColorRequests = Arc<Mutex<Vec<(usize, ColorFormatter)>>>;

#[derive(Clone)]
pub struct EventProxy {
    input: Input,
    title: Arc<Mutex<String>>,
    dimensions: Arc<Mutex<WindowSize>>,
    light: Arc<AtomicBool>,
    color_requests: ColorRequests,
    replies: Arc<Mutex<Vec<u8>>>,
    reply_overflow: Arc<AtomicBool>,
    reply_rate: Arc<Mutex<ReplyRate>>,
}
impl EventProxy {
    fn reply(&self, text: String) {
        let mut replies = self.replies.lock().unwrap_or_else(|p| p.into_inner());
        if replies.len().saturating_add(text.len()) <= REPLY_BUDGET {
            replies.extend_from_slice(text.as_bytes());
        } else {
            self.reply_overflow.store(true, Ordering::Release);
        }
    }
    fn flush_replies(&self) -> Result<(), String> {
        if self.input.closed.load(Ordering::Acquire) {
            self.replies
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clear();
            return Ok(());
        }
        if self.reply_overflow.swap(false, Ordering::AcqRel) {
            self.replies
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clear();
            return Err("터미널의 자동 응답이 안전 한도를 초과해 세션을 중단했습니다.".into());
        }
        let replies = std::mem::take(&mut *self.replies.lock().unwrap_or_else(|p| p.into_inner()));
        if !replies.is_empty()
            && !self
                .reply_rate
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .admit(replies.len(), Instant::now())
        {
            return Err("터미널 자동 응답이 반복돼 해당 세션을 중단했습니다.".into());
        }
        if replies.is_empty() {
            Ok(())
        } else {
            self.input.enqueue(replies, true)
        }
    }
}
impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                self.reply(text);
            }
            Event::Title(title) => {
                *self.title.lock().unwrap_or_else(|p| p.into_inner()) = title
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(128)
                    .collect();
            }
            Event::ResetTitle => {
                self.title.lock().unwrap_or_else(|p| p.into_inner()).clear();
            }
            Event::TextAreaSizeRequest(format) => {
                let size = *self.dimensions.lock().unwrap_or_else(|p| p.into_inner());
                self.reply(format(size));
            }
            Event::ColorRequest(index, format) => {
                let mut requests = self
                    .color_requests
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                if requests.len() < REPLY_BUDGET / 32 {
                    requests.push((index, format));
                } else {
                    self.reply_overflow.store(true, Ordering::Release);
                }
            }
            _ => {}
        }
    }
}

fn parse_output(
    processor: &mut Processor,
    term: &mut Term<EventProxy>,
    proxy: &EventProxy,
    bytes: &[u8],
) {
    // Resolve queries at OSC terminators, before a later escape changes the palette.
    // The parser already owns the VT lock; callbacks must never try to lock it again.
    for chunk in bytes.split_inclusive(|byte| matches!(*byte, 7 | b'\\')) {
        processor.advance(term, chunk);
        reply_to_color_requests(term, proxy);
    }
}

fn reply_to_color_requests(term: &Term<EventProxy>, proxy: &EventProxy) {
    let requests = std::mem::take(
        &mut *proxy
            .color_requests
            .lock()
            .unwrap_or_else(|p| p.into_inner()),
    );
    for (index, format) in requests {
        let rgb = term.colors()[index]
            .unwrap_or_else(|| crate::palette::rgb(index, proxy.light.load(Ordering::Acquire)));
        proxy.reply(format(rgb));
    }
}

struct OutputParser {
    term: Arc<FairMutex<Term<EventProxy>>>,
    proxy: EventProxy,
    revision: Arc<AtomicU64>,
    repaint_pending: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    ctx: Context,
    transport: Option<Arc<Transport>>,
}

impl OutputParser {
    fn repaint(&self) {
        self.revision.fetch_add(1, Ordering::Release);
        if self.visible.load(Ordering::Acquire)
            && !self.repaint_pending.swap(true, Ordering::AcqRel)
        {
            self.ctx.request_repaint_after(Duration::from_millis(8));
        }
    }

    fn finish_sync(&self, processor: &mut Processor) -> Result<(), String> {
        let mut term = self.term.lock();
        processor.stop_sync(&mut *term);
        reply_to_color_requests(&term, &self.proxy);
        drop(term);
        self.proxy.flush_replies()?;
        self.repaint();
        Ok(())
    }

    fn run(self, output: Receiver<Vec<u8>>) {
        let mut processor: Processor = Processor::new();
        let mut input_mode = crate::win32_input::Decoder::default();
        loop {
            // Wait indefinitely while idle, and use the VT core's deadline only
            // during a synchronized frame. Blocking pipe reads cannot expire it.
            let received = match processor.sync_timeout().sync_timeout() {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        if let Err(error) = self.finish_sync(&mut processor) {
                            if let Some(transport) = &self.transport {
                                transport.stop(Some(error));
                            }
                            break;
                        }
                        continue;
                    }
                    output.recv_timeout(remaining)
                }
                None => output.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(bytes) => {
                    input_mode.advance(&bytes);
                    self.proxy
                        .input
                        .win32
                        .store(input_mode.active, Ordering::Release);
                    parse_output(&mut processor, &mut self.term.lock(), &self.proxy, &bytes);
                    if let Err(error) = self.proxy.flush_replies() {
                        if let Some(transport) = &self.transport {
                            transport.stop(Some(error));
                        }
                        break;
                    }
                    if processor.sync_bytes_count() < bytes.len() {
                        self.repaint();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if let Err(error) = self.finish_sync(&mut processor) {
                        if let Some(transport) = &self.transport {
                            transport.stop(Some(error));
                        }
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    if processor.sync_timeout().sync_timeout().is_some() {
                        let _ = self.finish_sync(&mut processor);
                    }
                    break;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct GridSize {
    pub cols: usize,
    pub rows: usize,
}
impl Dimensions for GridSize {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

pub struct Session {
    pub id: String,
    pub label: String,
    pub pid: Option<u32>,
    pub term: Arc<FairMutex<Term<EventProxy>>>,
    pub input: Input,
    pub revision: Arc<AtomicU64>,
    pub repaint_pending: Arc<AtomicBool>,
    pub visible: Arc<AtomicBool>,
    pub exited: Arc<AtomicBool>,
    pub light: Arc<AtomicBool>,
    pub title: Arc<Mutex<String>>,
    pub error: Arc<Mutex<Option<String>>>,
    summary_cache: Mutex<(u64, crate::mini::Summary)>,
    dimensions: Arc<Mutex<WindowSize>>,
    transport: Arc<Transport>,
    prompt_files: crate::prompt::PromptFiles,
}

impl Session {
    pub fn start(profile: ShellProfile, scrollback: usize, ctx: Context) -> Result<Self, String> {
        Self::start_with_id(
            profile,
            scrollback,
            ctx,
            Uuid::new_v4().to_string(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(false)),
            Default::default(),
        )
    }
    pub fn start_with_id(
        profile: ShellProfile,
        scrollback: usize,
        ctx: Context,
        id: String,
        exited: Arc<AtomicBool>,
        light: Arc<AtomicBool>,
        prompt: crate::prompt::PromptPreferences,
    ) -> Result<Self, String> {
        if Uuid::parse_str(&id).is_err() {
            return Err("세션 ID를 확인하세요.".into());
        }
        let prompt_files = crate::prompt::PromptFiles::create(
            &crate::settings::data_directory()?.join("sessions"),
            &prompt,
        )?;
        if matches!(profile, ShellProfile::GitBash { .. }) {
            prompt_files.install_windows_bridge()?;
        }
        let pty::OpenPty {
            console,
            master,
            mut reader,
            mut writer,
            mut child,
            job,
        } = pty::open_with_prompt(&profile, &id, 80, 24, Some(prompt_files.directory()))?;
        let pid = child.process_id();
        let (tx, rx) = sync_channel::<PtyCommand>(16);
        let queued = Arc::new(AtomicUsize::new(0));
        let input = Input {
            sender: tx,
            queued: queued.clone(),
            win32: Arc::new(AtomicBool::new(false)),
            workers: Arc::new(crate::io_workers::Workers::default()),
            closed: Default::default(),
        };
        let dimensions = Arc::new(Mutex::new(WindowSize {
            num_cols: 80,
            num_lines: 24,
            cell_width: 8,
            cell_height: 18,
        }));
        let title = Arc::new(Mutex::new(String::new()));
        let applied_dimensions = Arc::new(Mutex::new(
            *dimensions.lock().unwrap_or_else(|p| p.into_inner()),
        ));
        let proxy = EventProxy {
            input: input.clone(),
            title: title.clone(),
            dimensions: applied_dimensions.clone(),
            light: light.clone(),
            color_requests: Default::default(),
            replies: Default::default(),
            reply_overflow: Default::default(),
            reply_rate: Default::default(),
        };
        let term = Arc::new(FairMutex::new(Term::new(
            crate::terminal_budget::config(scrollback, 80),
            &GridSize { cols: 80, rows: 24 },
            proxy.clone(),
        )));
        let revision = Arc::new(AtomicU64::new(1));
        let repaint_pending = Arc::new(AtomicBool::new(false));
        let visible = Arc::new(AtomicBool::new(true));
        let error = Arc::new(Mutex::new(None));
        let transport = Arc::new(Transport {
            input: input.clone(),
            console,
            job: Arc::new(job),
            error: error.clone(),
            ctx: ctx.clone(),
        });

        // Bound transport buffering to 16 chunks (256KiB). A separate parser
        // waits on output or an active synchronized-update deadline, never polls.
        let (output_tx, output_rx) = sync_channel(16);
        let (read_done, read_finished) = sync_channel(1);
        let start_error = |error: std::io::Error| {
            transport.stop(None);
            format!("터미널 작업 스레드를 시작하지 못했습니다: {error}")
        };
        let read_transport = transport.clone();
        input.workers.register(
            thread::Builder::new()
                .name("pty-read".into())
                .spawn(move || {
                    let mut buffer = vec![0; READ_CHUNK];
                    while !read_transport.input.workers.stopped() {
                        match reader.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(count) => {
                                if output_tx.send(buffer[..count].to_vec()).is_err() {
                                    break;
                                }
                            }
                            Err(error) => {
                                read_transport.stop(Some(format!(
                                    "터미널 출력 연결이 종료됐습니다: {error}"
                                )));
                                let _ = read_done.try_send(());
                                return;
                            }
                        }
                    }
                    read_transport.stop(None);
                    let _ = read_done.try_send(());
                })
                .map_err(start_error)?,
        );
        let output_parser = OutputParser {
            term: term.clone(),
            proxy,
            revision: revision.clone(),
            repaint_pending: repaint_pending.clone(),
            visible: visible.clone(),
            ctx: ctx.clone(),
            transport: Some(transport.clone()),
        };
        thread::Builder::new()
            .name("pty-parse".into())
            .spawn(move || output_parser.run(output_rx))
            .map_err(start_error)?;
        let write_ctx = ctx.clone();
        let write_term = term.clone();
        let write_revision = revision.clone();
        let write_transport = transport.clone();
        input.workers.register(
            thread::Builder::new()
                .name("pty-write".into())
                .spawn(move || {
                    // One ordered queue prevents a later resize from overtaking typed input,
                    // or input from reaching ConPTY before an earlier resize completes.
                    for command in rx {
                        if write_transport.input.closed.load(Ordering::Acquire)
                            || write_transport.input.workers.stopped()
                        {
                            break;
                        }
                        let result = match command {
                            PtyCommand::Shutdown => break,
                            PtyCommand::Write(data) => {
                                let result = writer.write_all(&data).and_then(|_| writer.flush());
                                write_transport.input.release(data.len());
                                result.map_err(|e| e.to_string())
                            }
                            PtyCommand::Resize(cols, rows) => {
                                pty::size(cols, rows).and_then(|size| {
                                    {
                                        let mut term = write_term.lock();
                                        crate::terminal_budget::before_resize(
                                            &mut term,
                                            scrollback,
                                            cols as usize,
                                        );
                                        term.resize(GridSize {
                                            cols: cols as usize,
                                            rows: rows as usize,
                                        });
                                        term.set_options(crate::terminal_budget::config(
                                            scrollback,
                                            cols as usize,
                                        ));
                                    }
                                    master.resize(size).map_err(|e| e.to_string())?;
                                    let mut applied = applied_dimensions
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner());
                                    applied.num_cols = cols;
                                    applied.num_lines = rows;
                                    write_revision.fetch_add(1, Ordering::Release);
                                    write_ctx.request_repaint();
                                    Ok(())
                                })
                            }
                        };
                        if let Err(error) = result {
                            write_transport.stop(Some(error));
                            break;
                        }
                    }
                })
                .map_err(start_error)?,
        );
        let wait_exit = exited.clone();
        let wait_error = error.clone();
        let wait_transport = transport.clone();
        let wait_label = profile.label();
        thread::Builder::new().name("pty-wait".into()).spawn(move || {
            match child.wait() {
                Ok(status) if !status.success() => {
                    wait_error.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(|| format!(
                        "{wait_label} 세션이 종료 코드 {}로 끝났습니다. 셸 경로·시작 폴더와 WSL 배포판 상태를 확인하세요.", status.exit_code()
                    ));
                }
                Err(error) => {
                    wait_error.lock().unwrap_or_else(|p| p.into_inner()).get_or_insert_with(|| error.to_string());
                }
                _ => {}
            }
            wait_exit.store(true, Ordering::Release);
            ctx.request_repaint();
            // The terminal's event proxy also owns an input sender. Explicitly
            // close transport rather than waiting for that reference cycle.
            // Let ClosePseudoConsole and the reader drain final output. Only a
            // stalled host is force-stopped after this bounded background wait.
            wait_transport.finish_input();
            if read_finished.recv_timeout(Duration::from_secs(2)).is_err() {
                wait_transport.stop(Some("셸 종료 뒤 콘솔 출력 연결이 끝나지 않아 중단했습니다.".into()));
            }
        }).map_err(start_error)?;
        Ok(Self {
            id,
            label: profile.label(),
            pid,
            term,
            input,
            revision,
            repaint_pending,
            visible,
            exited,
            light,
            title,
            error,
            summary_cache: Mutex::new((0, Default::default())),
            dimensions,
            transport,
            prompt_files,
        })
    }

    pub fn update_prompt(
        &self,
        preferences: &crate::prompt::PromptPreferences,
    ) -> Result<(), String> {
        self.prompt_files.update(preferences)
    }

    pub fn summary(&self) -> crate::mini::Summary {
        let revision = self.revision.load(Ordering::Acquire);
        let mut cache = self.summary_cache.lock().unwrap_or_else(|p| p.into_inner());
        if cache.0 != revision {
            cache.1 = crate::mini::summarize(&*self.term.lock());
            cache.0 = revision;
        }
        cache.1.clone()
    }

    pub fn resize(&self, cols: u16, rows: u16, cell_width: u16, cell_height: u16) {
        if pty::size(cols, rows).is_err() || self.input.workers.stopped() {
            return;
        }
        let mut dimensions = self.dimensions.lock().unwrap_or_else(|p| p.into_inner());
        if dimensions.num_cols == cols && dimensions.num_lines == rows {
            dimensions.cell_width = cell_width;
            dimensions.cell_height = cell_height;
            return;
        }
        if self
            .input
            .sender
            .try_send(PtyCommand::Resize(cols, rows))
            .is_ok()
        {
            *dimensions = WindowSize {
                num_cols: cols,
                num_lines: rows,
                cell_width,
                cell_height,
            };
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.visible.store(false, Ordering::Release);
        self.transport.stop(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::term::Config;

    #[test]
    #[ignore = "Requires an isolated native Codex capture in MINITERM_CODEX_UI_CAPTURE"]
    fn native_codex_menu_capture_matches_reference_terminal_after_resize() {
        use alacritty_terminal::{
            index::{Column, Line},
            term::cell::Flags,
            vte::ansi::Color,
        };
        #[derive(serde::Deserialize)]
        struct ReferenceColor {
            rgb: Option<u32>,
            index: Option<usize>,
        }
        #[derive(serde::Deserialize)]
        struct Style {
            row: usize,
            column: usize,
            bold: bool,
            dim: bool,
            italic: bool,
            inverse: bool,
            fg: Option<ReferenceColor>,
            bg: Option<ReferenceColor>,
        }
        #[derive(serde::Deserialize)]
        struct Checkpoint {
            #[serde(default)]
            name: String,
            offset: usize,
            columns: usize,
            rows: usize,
            #[serde(default)]
            resize: bool,
            #[serde(default)]
            screen: Vec<String>,
            #[serde(default)]
            styles: Vec<Style>,
            #[serde(default)]
            cursor: Option<[usize; 2]>,
        }
        fn assert_color(actual: Color, expected: ReferenceColor, label: &str) {
            if let Some(rgb) = expected.rgb {
                let Color::Spec(actual) = actual else {
                    panic!("{label}: expected RGB, got {actual:?}");
                };
                let packed = ((actual.r as u32) << 16) | ((actual.g as u32) << 8) | actual.b as u32;
                assert_eq!(packed, rgb, "{label}");
            } else if let Some(index) = expected.index {
                let actual = match actual {
                    Color::Indexed(index) => index as usize,
                    Color::Named(index) => index as usize,
                    _ => panic!("{label}: expected palette index, got {actual:?}"),
                };
                assert_eq!(actual, index, "{label}");
            }
        }
        let root = std::path::PathBuf::from(
            std::env::var_os("MINITERM_CODEX_UI_CAPTURE").expect("capture directory"),
        );
        let checkpoints: Vec<Checkpoint> =
            serde_json::from_slice(&std::fs::read(root.join("checkpoints.json")).unwrap()).unwrap();
        let bytes = std::fs::read(root.join("output.vt")).unwrap();
        let parser = output_fixture();
        let mut processor = Processor::new();
        let mut term = parser.term.lock();
        let first = &checkpoints[0];
        term.resize(GridSize {
            cols: first.columns,
            rows: first.rows,
        });
        let mut offset = 0;
        for checkpoint in checkpoints {
            parse_output(
                &mut processor,
                &mut term,
                &parser.proxy,
                &bytes[offset..checkpoint.offset],
            );
            offset = checkpoint.offset;
            if checkpoint.resize {
                term.resize(GridSize {
                    cols: checkpoint.columns,
                    rows: checkpoint.rows,
                });
                continue;
            }
            assert!(
                processor.sync_timeout().sync_timeout().is_none(),
                "Uncommitted frame at {}",
                checkpoint.name
            );
            if let Some([column, row]) = checkpoint.cursor {
                assert_eq!(
                    term.grid().cursor.point.column.0,
                    column.min(checkpoint.columns - 1),
                    "{} cursor column",
                    checkpoint.name
                );
                assert_eq!(
                    term.grid().cursor.point.line.0 as usize,
                    row,
                    "{} cursor row",
                    checkpoint.name
                );
            }
            for row in 0..checkpoint.rows {
                let mut actual = String::new();
                for col in 0..checkpoint.columns {
                    let cell = &term.grid()[Line(row as i32)][Column(col)];
                    if !cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                        actual.push(cell.c);
                        if let Some(extra) = cell.zerowidth() {
                            actual.extend(extra);
                        }
                    }
                }
                assert_eq!(
                    actual.trim_end(),
                    checkpoint.screen[row].trim_end(),
                    "{} row {}",
                    checkpoint.name,
                    row
                );
            }
            // Real installed AI Battery captures place their row below Codex's
            // editable prompt. Summary selection must retain that actual row.
            if let Some(row) = checkpoint
                .screen
                .iter()
                .rposition(|text| text.contains("AI Battery ❚") && text.contains('%'))
            {
                let summary = crate::mini::summarize(&term);
                assert_eq!(
                    summary.battery_row,
                    Some(row as i32),
                    "{} summary footer",
                    checkpoint.name
                );
                assert_eq!(
                    summary.battery_text.as_deref(),
                    Some(checkpoint.screen[row].trim()),
                    "{} battery text",
                    checkpoint.name
                );
                assert!(
                    summary.plan.is_none(),
                    "No plan was provided in the native menu fixture"
                );
            }
            for style in checkpoint.styles {
                let cell = &term.grid()[Line(style.row as i32)][Column(style.column)];
                let label = format!("{} cell {},{}", checkpoint.name, style.row, style.column);
                for (flag, expected) in [
                    (Flags::BOLD, style.bold),
                    (Flags::DIM, style.dim),
                    (Flags::ITALIC, style.italic),
                    (Flags::INVERSE, style.inverse),
                ] {
                    assert_eq!(cell.flags.contains(flag), expected, "{label} {flag:?}");
                }
                if let Some(expected) = style.fg {
                    assert_color(cell.fg, expected, &label);
                }
                if let Some(expected) = style.bg {
                    assert_color(cell.bg, expected, &label);
                }
            }
        }
    }

    fn output_fixture() -> OutputParser {
        let (sender, _) = sync_channel(16);
        let proxy = EventProxy {
            input: Input {
                sender,
                queued: Default::default(),
                win32: Arc::new(AtomicBool::new(false)),
                workers: Default::default(),
                closed: Default::default(),
            },
            title: Default::default(),
            dimensions: Arc::new(Mutex::new(WindowSize {
                num_cols: 40,
                num_lines: 4,
                cell_width: 8,
                cell_height: 18,
            })),
            light: Default::default(),
            color_requests: Default::default(),
            replies: Default::default(),
            reply_overflow: Default::default(),
            reply_rate: Default::default(),
        };
        OutputParser {
            term: Arc::new(FairMutex::new(Term::new(
                Config::default(),
                &GridSize { cols: 40, rows: 4 },
                proxy.clone(),
            ))),
            proxy,
            revision: Default::default(),
            repaint_pending: Default::default(),
            visible: Arc::new(AtomicBool::new(true)),
            ctx: Context::default(),
            transport: None,
        }
    }

    fn bottom_text(term: &FairMutex<Term<EventProxy>>) -> String {
        use alacritty_terminal::index::{Column, Line};
        let term = term.lock();
        (0..4)
            .map(|col| term.grid()[Line(3)][Column(col)].c)
            .collect()
    }

    fn wait_for_bottom(term: &FairMutex<Term<EventProxy>>, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while bottom_text(term) != text {
            assert!(Instant::now() < deadline, "Missing bottom row: {text}");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn protocol_backpressure_releases_the_vt_lock_before_waiting_for_a_writer() {
        let mut parser = output_fixture();
        let (input, written) = sync_channel(1);
        parser.proxy.input.sender = input;
        parser.proxy.input.send(b"i".to_vec()).unwrap();
        let queued = parser.proxy.input.queued.clone();
        let term = parser.term.clone();
        let (output, bytes) = sync_channel(1);
        let parser = thread::spawn(move || parser.run(bytes));
        output.send(b"\x1b[6n".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while queued.load(Ordering::Acquire) <= 1 {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        let (done, received) = std::sync::mpsc::channel();
        let writer = thread::spawn(move || {
            let _guard = term.lock();
            assert!(matches!(written.recv().unwrap(), PtyCommand::Write(_)));
            let reply = match written.recv().unwrap() {
                PtyCommand::Write(bytes) => bytes,
                _ => panic!("Missing reply"),
            };
            done.send(reply).unwrap();
        });
        assert_eq!(
            received.recv_timeout(Duration::from_secs(2)).unwrap(),
            b"\x1b[1;1R"
        );
        drop(output);
        writer.join().unwrap();
        parser.join().unwrap();
    }

    #[test]
    fn automatic_response_floods_are_bounded_and_normal_bursts_recover() {
        let mut rate = ReplyRate::default();
        let now = rate.updated;
        assert!(rate.admit(REPLY_BURST as usize, now));
        assert!(!rate.admit(1, now));
        assert!(rate.admit(REPLY_PER_SECOND as usize, now + Duration::from_secs(1)));
        assert!(!rate.admit(REPLY_BURST as usize + 1, now + Duration::from_secs(100)));
        let parser = output_fixture();
        parser.proxy.reply("x".repeat(REPLY_BUDGET + 1));
        assert!(parser.proxy.flush_replies().is_err());
        assert!(parser.proxy.replies.lock().unwrap().is_empty());
    }

    #[test]
    #[ignore = "Requires isolated MINITERM_DATA_DIR and installed Git Bash; no GUI/model"]
    fn native_console_limits_shutdown_and_final_output_are_preserved() {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0,
            System::Threading::{OpenProcess, WaitForSingleObject},
        };
        let root =
            std::env::var_os("MINITERM_DATA_DIR").expect("Run with an isolated fixture directory");
        std::fs::create_dir_all(&root).unwrap();
        let profile = ShellProfile::GitBash {
            bash_path: None,
            directory: Some(root.into()),
        };
        let start = || Session::start(profile.clone(), 50_000, Context::default()).unwrap();
        let session = start();
        assert_eq!(
            session.transport.console.memory_limit(),
            crate::console_guard::CONSOLE_COMMIT_LIMIT
        );
        assert_eq!(session.transport.job.process_memory_limit(), 0);
        let handle = unsafe { OpenProcess(0x0010_1000, 0, session.pid.unwrap()) };
        assert!(!handle.is_null());
        let handle = unsafe { OwnedHandle::from_raw_handle(handle as _) };
        assert!(!session
            .transport
            .console
            .contains(handle.as_raw_handle() as isize));
        session.input.send(b"HISTFILE=/dev/null; for ((i=0;i<1000;i++)); do printf 'FINAL-OUTPUT-LINE\\n'; done; printf 'TAIL-''PRESERVED\\n'; exit\r".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let text: String = session
                .term
                .lock()
                .grid()
                .display_iter()
                .map(|indexed| indexed.cell.c)
                .collect();
            if session.exited.load(Ordering::Acquire) && text.contains("TAIL-PRESERVED") {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Final shell output did not drain"
            );
            thread::sleep(Duration::from_millis(10));
        }
        drop(session);
        assert_eq!(
            unsafe { WaitForSingleObject(handle.as_raw_handle() as _, 2000) },
            WAIT_OBJECT_0
        );
        let session = start();
        let term = session.term.clone();
        let input = session.input.clone();
        let console = unsafe { OpenProcess(0x0010_1000, 0, session.transport.console.pid) };
        assert!(!console.is_null());
        let console = unsafe { OwnedHandle::from_raw_handle(console as _) };
        input.send(b"HISTFILE=/dev/null; for ((i=0;i<100000;i++)); do printf 'BUSY-OUTPUT-LINE\\n'; done\r".to_vec()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while session.revision.load(Ordering::Acquire) < 8 {
            assert!(Instant::now() < deadline, "No output before shutdown");
            thread::sleep(Duration::from_millis(5));
        }
        let closing = Instant::now();
        drop(session);
        assert!(closing.elapsed() < Duration::from_secs(1));
        assert_eq!(
            unsafe { WaitForSingleObject(console.as_raw_handle() as _, 2000) },
            WAIT_OBJECT_0
        );
        while Arc::strong_count(&term) != 1 {
            assert!(
                closing.elapsed() < Duration::from_secs(3),
                "Output worker retained terminal state"
            );
            thread::sleep(Duration::from_millis(5));
        }
        assert!(input.send(b"late".to_vec()).is_err());
        assert_eq!(input.queued.load(Ordering::Acquire), 0);
    }

    #[test]
    fn unfinished_synchronized_menu_expires_without_more_output_or_eof() {
        let parser = output_fixture();
        let term = parser.term.clone();
        let (tx, rx) = sync_channel(16);
        let worker = thread::spawn(move || parser.run(rx));
        tx.send(b"\x1b[4;1HOLD!".to_vec()).unwrap();
        wait_for_bottom(&term, "OLD!");
        tx.send(b"\x1b[?2026h\x1b[4;1H\x1b[2KMENU".to_vec())
            .unwrap();
        // A partial frame must stay hidden before the core's 150ms deadline.
        thread::sleep(Duration::from_millis(40));
        assert_eq!(bottom_text(&term), "OLD!");
        // Keep tx open: recovery must not rely on another chunk or process exit.
        wait_for_bottom(&term, "MENU");
        drop(tx);
        worker.join().unwrap();
    }

    #[test]
    fn synchronized_bottom_menu_commits_across_pipe_chunks() {
        let parser = output_fixture();
        let term = parser.term.clone();
        let (tx, rx) = sync_channel(16);
        let worker = thread::spawn(move || parser.run(rx));
        tx.send(b"\x1b[?2026h\x1b[4;1HME".to_vec()).unwrap();
        tx.send(b"NU\x1b[?2026l".to_vec()).unwrap();
        wait_for_bottom(&term, "MENU");
        drop(tx);
        worker.join().unwrap();
    }

    #[test]
    fn eof_preserves_a_final_unfinished_synchronized_frame() {
        let parser = output_fixture();
        let term = parser.term.clone();
        let (tx, rx) = sync_channel(16);
        let worker = thread::spawn(move || parser.run(rx));
        tx.send(b"\x1b[?2026h\x1b[4;1HMENU".to_vec()).unwrap();
        drop(tx);
        worker.join().unwrap();
        assert_eq!(bottom_text(&term), "MENU");
    }

    #[test]
    fn osc_queries_reply_with_the_color_at_that_point_in_the_stream() {
        let (tx, rx) = sync_channel(16);
        let proxy = EventProxy {
            input: Input {
                sender: tx,
                queued: Default::default(),
                win32: Arc::new(AtomicBool::new(false)),
                workers: Default::default(),
                closed: Default::default(),
            },
            title: Default::default(),
            dimensions: Arc::new(Mutex::new(WindowSize {
                num_cols: 80,
                num_lines: 4,
                cell_width: 8,
                cell_height: 18,
            })),
            light: Arc::new(AtomicBool::new(true)),
            color_requests: Default::default(),
            replies: Default::default(),
            reply_overflow: Default::default(),
            reply_rate: Default::default(),
        };
        let mut term = Term::new(
            Config::default(),
            &GridSize { cols: 80, rows: 4 },
            proxy.clone(),
        );
        let mut processor = Processor::new();
        parse_output(
            &mut processor,
            &mut term,
            &proxy,
            b"\x1b]10;#123456\x07\x1b]10;?\x07\x1b]110\x07\x1b]10;?\x1b",
        );
        // An ST terminator may cross a PTY read boundary.
        parse_output(&mut processor, &mut term, &proxy, b"\\\x1b]10;#ffffff\x07");
        proxy.flush_replies().unwrap();
        let receive = || match rx.try_recv().unwrap() {
            PtyCommand::Write(data) => String::from_utf8(data).unwrap(),
            PtyCommand::Resize(_, _) | PtyCommand::Shutdown => panic!("Unexpected control command"),
        };
        assert_eq!(
            receive(),
            "\x1b]10;rgb:1212/3434/5656\x07\x1b]10;rgb:2626/2f2f/3c3c\x1b\\"
        );
        assert!(rx.try_recv().is_err());
    }
}
