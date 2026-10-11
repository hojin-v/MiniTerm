use crate::{pty, shell::ShellProfile};
use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    grid::Dimensions,
    sync::FairMutex,
    term::{Config, Osc52, Term},
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

enum PtyCommand {
    Write(Vec<u8>),
    Resize(u16, u16),
}

#[derive(Clone)]
pub struct Input {
    sender: SyncSender<PtyCommand>,
    queued: Arc<AtomicUsize>,
}
impl Input {
    pub fn send(&self, data: Vec<u8>) -> Result<(), String> {
        let count = data.len();
        if count > INPUT_BUDGET {
            return Err("붙여넣기는 1MB 이하로 나누어 주세요.".into());
        }
        self.queued
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |queued| {
                (queued + count <= INPUT_BUDGET).then_some(queued + count)
            })
            .map_err(|_| "입력 큐가 가득 찼습니다.")?;
        if self.sender.try_send(PtyCommand::Write(data)).is_err() {
            self.queued.fetch_sub(count, Ordering::AcqRel);
            return Err("입력 연결이 닫혔거나 큐가 가득 찼습니다.".into());
        }
        Ok(())
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
}
impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => {
                let _ = self.input.send(text.into_bytes());
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
                let _ = self.input.send(format(size).into_bytes());
            }
            Event::ColorRequest(index, format) => {
                self.color_requests
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .push((index, format));
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
        let _ = proxy.input.send(format(rgb).into_bytes());
    }
}

struct OutputParser {
    term: Arc<FairMutex<Term<EventProxy>>>,
    proxy: EventProxy,
    revision: Arc<AtomicU64>,
    repaint_pending: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    ctx: Context,
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

    fn finish_sync(&self, processor: &mut Processor) {
        let mut term = self.term.lock();
        processor.stop_sync(&mut *term);
        reply_to_color_requests(&term, &self.proxy);
        drop(term);
        self.repaint();
    }

    fn run(self, output: Receiver<Vec<u8>>) {
        let mut processor: Processor = Processor::new();
        loop {
            // Wait indefinitely while idle, and use the VT core's deadline only
            // during a synchronized frame. Blocking pipe reads cannot expire it.
            let received = match processor.sync_timeout().sync_timeout() {
                Some(deadline) => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        self.finish_sync(&mut processor);
                        continue;
                    }
                    output.recv_timeout(remaining)
                }
                None => output.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(bytes) => {
                    parse_output(&mut processor, &mut self.term.lock(), &self.proxy, &bytes);
                    if processor.sync_bytes_count() < bytes.len() {
                        self.repaint();
                    }
                }
                Err(RecvTimeoutError::Timeout) => self.finish_sync(&mut processor),
                Err(RecvTimeoutError::Disconnected) => {
                    if processor.sync_timeout().sync_timeout().is_some() {
                        self.finish_sync(&mut processor);
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
    job: crate::native::ProcessJob,
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
        };
        let dimensions = Arc::new(Mutex::new(WindowSize {
            num_cols: 80,
            num_lines: 24,
            cell_width: 8,
            cell_height: 18,
        }));
        let title = Arc::new(Mutex::new(String::new()));
        let proxy = EventProxy {
            input: input.clone(),
            title: title.clone(),
            dimensions: dimensions.clone(),
            light: light.clone(),
            color_requests: Default::default(),
        };
        let term = Arc::new(FairMutex::new(Term::new(
            Config {
                scrolling_history: scrollback,
                osc52: Osc52::Disabled,
                ..Config::default()
            },
            &GridSize { cols: 80, rows: 24 },
            proxy.clone(),
        )));
        let revision = Arc::new(AtomicU64::new(1));
        let repaint_pending = Arc::new(AtomicBool::new(false));
        let visible = Arc::new(AtomicBool::new(true));
        let error = Arc::new(Mutex::new(None));

        // Bound transport buffering to 16 chunks (256KiB). A separate parser
        // waits on output or an active synchronized-update deadline, never polls.
        let (output_tx, output_rx) = sync_channel(16);
        thread::spawn(move || {
            let mut buffer = vec![0; READ_CHUNK];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 || output_tx.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });
        let output_parser = OutputParser {
            term: term.clone(),
            proxy,
            revision: revision.clone(),
            repaint_pending: repaint_pending.clone(),
            visible: visible.clone(),
            ctx: ctx.clone(),
        };
        thread::spawn(move || output_parser.run(output_rx));
        let write_error = error.clone();
        let write_ctx = ctx.clone();
        let write_term = term.clone();
        let write_revision = revision.clone();
        thread::spawn(move || {
            // One ordered queue prevents a later resize from overtaking typed input,
            // or input from reaching ConPTY before an earlier resize completes.
            for command in rx {
                let writing = matches!(command, PtyCommand::Write(_));
                let result = match command {
                    PtyCommand::Write(data) => {
                        let result = writer.write_all(&data).and_then(|_| writer.flush());
                        queued.fetch_sub(data.len(), Ordering::AcqRel);
                        result.map_err(|e| e.to_string())
                    }
                    PtyCommand::Resize(cols, rows) => pty::size(cols, rows).and_then(|size| {
                        write_term.lock().resize(GridSize {
                            cols: cols as usize,
                            rows: rows as usize,
                        });
                        master.resize(size).map_err(|e| e.to_string())?;
                        write_revision.fetch_add(1, Ordering::Release);
                        write_ctx.request_repaint();
                        Ok(())
                    }),
                };
                if let Err(error) = result {
                    *write_error.lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(error.to_string());
                    write_ctx.request_repaint();
                    if writing {
                        break;
                    }
                }
            }
        });
        let wait_exit = exited.clone();
        let wait_error = error.clone();
        thread::spawn(move || {
            if let Err(error) = child.wait() {
                *wait_error.lock().unwrap_or_else(|p| p.into_inner()) = Some(error.to_string());
            }
            wait_exit.store(true, Ordering::Release);
            ctx.request_repaint();
        });
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
            job,
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
        self.job.terminate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let receive = || match rx.try_recv().unwrap() {
            PtyCommand::Write(data) => String::from_utf8(data).unwrap(),
            PtyCommand::Resize(_, _) => panic!("Unexpected resize"),
        };
        assert_eq!(receive(), "\x1b]10;rgb:1212/3434/5656\x07");
        assert_eq!(receive(), "\x1b]10;rgb:2626/2f2f/3c3c\x1b\\");
        assert!(rx.try_recv().is_err());
    }
}
