use crate::{
    agent::Broker,
    icons::{Icon as ActionIcon, IconButton},
    ipc, native, notification,
    settings::Settings,
    shell::ShellProfile,
    terminal::Session,
};
use alacritty_terminal::{
    grid::{Dimensions, Scroll},
    index::{Column, Line, Point, Side},
    selection::{Selection, SelectionType},
    term::{cell::Flags, TermMode},
    vte::ansi::{Color, CursorShape, NamedColor},
};
use eframe::egui::{
    self, Color32, Context, Event, FontData, FontDefinitions, FontFamily, FontId, ImeEvent, Key,
    Pos2, Rect, Sense, Shape, Stroke, Vec2,
};
use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
        Arc, LazyLock, Mutex,
    },
    thread,
    time::Instant,
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

enum Action {
    Show,
    Hide,
    Settings,
    Quit,
    SmokeExit,
    Diagnostic(String),
    Activate(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HudShortcut {
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    SelectTab(usize),
    Settings,
    ToggleExpanded,
    ToggleMiniView,
    FontSize(i16),
    ScrollLines(i32),
    ScrollPage(bool),
    ScrollEnd(bool),
}
#[derive(Default)]
struct RowCache {
    hash: u64,
    shapes: Vec<Shape>,
}

struct WindowTransition {
    from: crate::geometry::Rect,
    to: crate::geometry::Rect,
    started: Instant,
}
type DiagnosticUi = Arc<Mutex<BTreeMap<String, [f32; 4]>>>;

const SETTINGS_SIZE: Vec2 = Vec2::new(560.0, 720.0);

fn settings_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("settings")
}

#[derive(Default)]
struct FrameGate(Option<u64>);
impl FrameGate {
    fn claim(&mut self, frame: u64) -> bool {
        if self.0 == Some(frame) {
            false
        } else {
            self.0 = Some(frame);
            true
        }
    }
}

pub struct MiniTerm {
    hwnd: isize,
    settings: Settings,
    saved_settings: Settings,
    edited_settings: Settings,
    settings_observed: Settings,
    settings_error: Option<String>,
    integration_feedback: Option<(bool, String)>,
    settings_path: PathBuf,
    sessions: Vec<Session>,
    selected: usize,
    starting: usize,
    pending_activation: Option<String>,
    started_tx: Sender<Result<Session, String>>,
    started_rx: Receiver<Result<Session, String>>,
    actions: Receiver<Action>,
    _tray: TrayIcon,
    display_watch: native::DisplayWatch,
    appearance: native::WindowAppearance,
    expanded: bool,
    transition: Option<WindowTransition>,
    was_focused: bool,
    terminal_focus: bool,
    settings_open: bool,
    settings_native_ready: bool,
    settings_position: Option<Pos2>,
    quit_confirm: bool,
    close_confirm: Option<usize>,
    quit: bool,
    error: Option<String>,
    composition: String,
    composing: bool,
    rows: Vec<RowCache>,
    atlas_reference: Option<Arc<egui::Galley>>,
    layout_key: (usize, u32, u32, bool, usize, u32, u32),
    last_scale: f64,
    bash_path: String,
    bash_directory: String,
    distribution: String,
    wsl_user: String,
    wsl_directory: String,
    settings_tab: usize,
    available_monitors: Vec<native::Monitor>,
    available_distributions: Vec<crate::shell::WslDistribution>,
    wsl: bool,
    frame_samples: Vec<f64>,
    adapter_name: String,
    repaint_causes: BTreeMap<String, u64>,
    startup_focus_preserved: bool,
    header_gate: FrameGate,
    input_gate: FrameGate,
    scroll_gate: FrameGate,
    shortcut_gate: FrameGate,
    opacity_hover: Option<Instant>,
    opacity_keepalive: Option<Instant>,
    opacity_open: bool,
    opacity_anchor: Pos2,
    transparency_save_at: Option<Instant>,
    brand: egui::TextureHandle,
    settings_gate: FrameGate,
    broker: Arc<Mutex<Broker>>,
    _ipc_server: ipc::Server,
    toast_worker: Arc<notification::ToastWorker>,
    notification_preferences: Arc<Mutex<(bool, bool)>>,
    notices_open: bool,
    diagnostic_ui: Option<DiagnosticUi>,
    diagnostic_scroll: Option<Arc<Mutex<ipc::TerminalScroll>>>,
    diagnostic_sample_requested: bool,
    diagnostic_settings_keys: Vec<Event>,
    diagnostic_sample_ready: Arc<AtomicBool>,
}

impl MiniTerm {
    fn record_widget(&self, name: &str, rect: Rect) {
        if let Some(map) = &self.diagnostic_ui {
            map.lock().unwrap_or_else(|p| p.into_inner()).insert(
                name.into(),
                [rect.left(), rect.top(), rect.right(), rect.bottom()],
            );
        }
    }
    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let ctx = &cc.egui_ctx;
        ctx.style_mut(|style| {
            style.animation_time = 0.0;
            style.spacing.item_spacing = Vec2::new(6.0, 5.0);
        });
        let hwnd = native::hwnd(cc)?;
        let path = crate::settings::data_directory()?.join("settings.json");
        let (settings, warning) = match Settings::load(&path) {
            Ok(settings) => (settings, None),
            Err(error) => (Settings::default(), Some(error)),
        };
        install_fonts(ctx, &settings.font_family);
        // Terminal font shortcuts must not also scale the entire egui interface.
        ctx.options_mut(|options| options.zoom_with_keyboard = false);
        let (actions_tx, actions) = channel();
        let broker = Arc::new(Mutex::new(Broker::default()));
        let notification_preferences = Arc::new(Mutex::new((
            settings.notifications,
            settings.notification_sound,
        )));
        let activation_tx = actions_tx.clone();
        let activation_ctx = ctx.clone();
        let toast_worker = Arc::new(notification::ToastWorker::new(move |id| {
            native::show(hwnd, true);
            let _ = activation_tx.send(Action::Activate(id));
            activation_ctx.request_repaint();
        }));
        let ipc_broker = broker.clone();
        let ipc_preferences = notification_preferences.clone();
        let ipc_toast = toast_worker.clone();
        let ipc_tx = actions_tx.clone();
        let ipc_ctx = ctx.clone();
        let diagnostics = std::env::var("MINITERM_DIAGNOSTICS").as_deref() == Ok("1");
        let diagnostic_ui = diagnostics.then(|| Arc::new(Mutex::new(BTreeMap::new())));
        let ipc_ui = diagnostic_ui.clone();
        let isolated = diagnostics && std::env::var_os("MINITERM_INSTANCE_ID").is_some();
        let diagnostic_scroll =
            isolated.then(|| Arc::new(Mutex::new(ipc::TerminalScroll::default())));
        let ipc_scroll = diagnostic_scroll.clone();
        let diagnostic_sample_ready = Arc::new(AtomicBool::new(false));
        let ipc_sample_ready = diagnostic_sample_ready.clone();
        let ipc_server = ipc::serve(move |request| {
            if let ipc::Request::CodexEvent { connection, event } = &request {
                if !crate::codex_link::active(&event.session_id, connection) {
                    return ipc::Reply::error("이 Codex 알림 연결은 종료되었습니다.");
                }
            }
            match request {
                ipc::Request::PrepareCodex {
                    session_id,
                    home,
                    cwd,
                    environment,
                } => {
                    let active = ipc_broker
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .sessions
                        .get(&session_id)
                        .is_some_and(|s| {
                            !s.exited.as_ref().is_some_and(|e| e.load(Ordering::Acquire))
                        });
                    if !active {
                        return ipc::Reply::error("Codex를 실행한 탭을 확인하세요.");
                    }
                    crate::codex_bash::begin_preparation(&session_id, &home, &cwd, environment)
                }
                ipc::Request::CodexPrepared { session_id, ticket } => {
                    crate::codex_bash::poll_preparation(&session_id, &ticket)
                }
                ipc::Request::CloseCodexLink {
                    session_id,
                    connection,
                } => {
                    crate::codex_link::close(&session_id, &connection);
                    ipc::Reply::ok()
                }
                ipc::Request::Diagnostic { command } => {
                    if !diagnostics {
                        return ipc::Reply::error("진단 모드가 활성화되지 않았습니다.");
                    }
                    let fixture_key = diagnostic_key(&command);
                    if ![
                        "hide",
                        "expand",
                        "collapse",
                        "settings",
                        "new-session",
                        "quit",
                        "theme-light",
                        "theme-dark",
                        "settings-close",
                        "settings-cancel",
                        "confirm-quit",
                        "sample",
                        "sample-ui",
                        "sample-summary",
                        "sample-summary-command",
                        "sample-scroll",
                    ]
                    .contains(&command.as_str())
                        && fixture_key.is_none()
                    {
                        return ipc::Reply::error("지원하지 않는 진단 명령입니다.");
                    }
                    if fixture_key.is_some() && !isolated {
                        return ipc::Reply::error(
                            "키 입력 시험은 별도 진단 세션에서만 사용할 수 있습니다.",
                        );
                    }
                    if matches!(
                        command.as_str(),
                        "sample"
                            | "sample-ui"
                            | "sample-summary"
                            | "sample-summary-command"
                            | "sample-scroll"
                            | "confirm-quit"
                    ) && std::env::var_os("MINITERM_INSTANCE_ID").is_none()
                    {
                        return ipc::Reply::error(
                            "출력 샘플은 별도 진단 세션에서만 사용할 수 있습니다.",
                        );
                    }
                    // Hidden viewports need a native visibility event to dispatch a UI action.
                    if !matches!(
                        command.as_str(),
                        "hide" | "settings" | "settings-close" | "settings-cancel"
                    ) {
                        native::show(hwnd, false);
                    }
                    let _ = ipc_tx.send(Action::Diagnostic(command));
                    ipc_ctx.request_repaint();
                    native::wake_hidden(hwnd);
                    ipc::Reply::ok()
                }
                ipc::Request::Event { event } | ipc::Request::CodexEvent { event, .. } => {
                    if let Err(error) = event.validate() {
                        return ipc::Reply::error(error);
                    }
                    let notice = {
                        let mut broker = ipc_broker.lock().unwrap_or_else(|p| p.into_inner());
                        if !broker.sessions.contains_key(&event.session_id) {
                            return ipc::Reply::error("이 세션은 이미 종료되었습니다.");
                        }
                        let observed = native::visible(hwnd)
                            && native::focused(hwnd)
                            && broker.selected.as_deref() == Some(&event.session_id);
                        broker.receive(event, observed)
                    };
                    let (enabled, sound) =
                        *ipc_preferences.lock().unwrap_or_else(|p| p.into_inner());
                    if enabled {
                        if let Some(notice) = notice {
                            ipc_toast.send(notice, sound);
                        }
                    }
                    ipc_ctx.request_repaint();
                    ipc::Reply::ok()
                }
                ipc::Request::Progress { session_id, update } => {
                    let result = ipc_broker
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .progress(&session_id, update);
                    match result {
                        Ok(progress) => {
                            ipc_ctx.request_repaint();
                            ipc::Reply {
                                progress,
                                ..ipc::Reply::ok()
                            }
                        }
                        Err(error) => ipc::Reply::error(error),
                    }
                }
                ipc::Request::Activate { session_id } => {
                    if uuid::Uuid::parse_str(&session_id).is_err() {
                        return ipc::Reply::error("알림 세션 ID를 확인하세요.");
                    }
                    native::show(hwnd, true);
                    let _ = ipc_tx.send(Action::Activate(session_id));
                    ipc_ctx.request_repaint();
                    ipc::Reply::ok()
                }
                ipc::Request::Show => {
                    native::show(hwnd, true);
                    let _ = ipc_tx.send(Action::Show);
                    ipc_ctx.request_repaint();
                    ipc::Reply::ok()
                }
                ipc::Request::Sessions => {
                    let broker = ipc_broker.lock().unwrap_or_else(|p| p.into_inner());
                    ipc::Reply {
                        ok: true,
                        sessions: broker
                            .sessions
                            .iter()
                            .filter(|(_, s)| {
                                !s.exited.as_ref().is_some_and(|e| e.load(Ordering::Acquire))
                            })
                            .map(|(id, _)| id.clone())
                            .collect(),
                        error: None,
                        progress: None,
                        capabilities: vec!["progress-v1".into()],
                        helper_capabilities: Vec::new(),
                        preparation_id: None,
                        codex_connection: None,
                        details: Some(ipc::Snapshot {
                            fixture_sample_ready: isolated
                                .then(|| ipc_sample_ready.load(Ordering::Acquire)),
                            states: broker
                                .sessions
                                .iter()
                                .map(|(id, s)| (id.clone(), s.state))
                                .collect(),
                            unread: broker
                                .sessions
                                .iter()
                                .filter(|(_, s)| s.unread)
                                .map(|(id, _)| id.clone())
                                .collect(),
                            notice_count: broker.notices.len(),
                            toast_count: ipc_toast.published.load(Ordering::Relaxed),
                            toast_error: ipc_toast
                                .warning
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .clone(),
                            ui: ipc_ui
                                .as_ref()
                                .map(|map| map.lock().unwrap_or_else(|p| p.into_inner()).clone()),
                            terminal_scroll: ipc_scroll.as_ref().map(|state| {
                                state.lock().unwrap_or_else(|p| p.into_inner()).clone()
                            }),
                        }),
                    }
                }
            }
        })?;
        if let Ok(seconds) = std::env::var("MINITERM_SMOKE_SECONDS") {
            if let Ok(seconds) = seconds.parse::<u64>() {
                let tx = actions_tx.clone();
                let ctx = ctx.clone();
                thread::spawn(move || {
                    thread::sleep(std::time::Duration::from_secs(seconds.min(300)));
                    native::show(hwnd, false);
                    let _ = tx.send(Action::SmokeExit);
                    ctx.request_repaint();
                });
            }
        }
        let menu = Menu::new();
        let show = MenuItem::new("MiniTerm 열기", true, None);
        let hide = MenuItem::new("숨기기", true, None);
        let preferences = MenuItem::new("설정", true, None);
        let quit = MenuItem::new("종료…", true, None);
        menu.append_items(&[&show, &hide, &preferences, &quit])?;
        let ids = [
            show.id().clone(),
            hide.id().clone(),
            preferences.id().clone(),
            quit.id().clone(),
        ];
        let menu_tx = actions_tx.clone();
        let menu_ctx = ctx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = if event.id == ids[0] {
                Action::Show
            } else if event.id == ids[1] {
                Action::Hide
            } else if event.id == ids[2] {
                Action::Settings
            } else {
                Action::Quit
            };
            if !matches!(action, Action::Hide | Action::Settings) {
                native::show(hwnd, true);
            }
            let _ = menu_tx.send(action);
            menu_ctx.request_repaint();
            native::wake_hidden(hwnd);
        }));
        let tray_tx = actions_tx.clone();
        let tray_ctx = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                native::show(hwnd, true);
                let _ = tray_tx.send(Action::Show);
                tray_ctx.request_repaint();
            }
        }));
        let tray = TrayIconBuilder::new()
            .with_tooltip("MiniTerm · Bash")
            .with_icon(tray_icon())
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .build()?;
        let display_watch = native::DisplayWatch::new(hwnd, ctx.clone())?;
        let hotkey_error = display_watch.register_shortcut(&settings.shortcut).err();
        let (started_tx, started_rx) = channel();
        let mut app = Self {
            hwnd,
            saved_settings: settings.clone(),
            edited_settings: settings.clone(),
            settings_observed: settings.clone(),
            settings_error: None,
            integration_feedback: None,
            settings,
            settings_path: path,
            sessions: Vec::new(),
            selected: 0,
            starting: 0,
            pending_activation: None,
            started_tx,
            started_rx,
            actions,
            _tray: tray,
            display_watch,
            appearance: Default::default(),
            expanded: false,
            transition: None,
            was_focused: false,
            terminal_focus: true,
            settings_open: false,
            settings_native_ready: false,
            settings_position: None,
            quit_confirm: false,
            close_confirm: None,
            quit: false,
            error: warning.or(hotkey_error).or_else(|| {
                std::env::args()
                    .any(|arg| arg == "--stale-notice")
                    .then(|| {
                        "이 알림의 세션은 이미 종료되었습니다. 이전 명령은 다시 실행하지 않습니다."
                            .into()
                    })
            }),
            composition: String::new(),
            composing: false,
            rows: Vec::new(),
            atlas_reference: None,
            layout_key: (0, 0, 0, false, usize::MAX, 0, 0),
            last_scale: ctx.pixels_per_point() as f64,
            bash_path: String::new(),
            bash_directory: String::new(),
            distribution: String::new(),
            wsl_user: String::new(),
            wsl_directory: "~".into(),
            settings_tab: 0,
            available_monitors: Vec::new(),
            available_distributions: Vec::new(),
            wsl: false,
            frame_samples: Vec::new(),
            repaint_causes: BTreeMap::new(),
            startup_focus_preserved: false,
            header_gate: FrameGate::default(),
            input_gate: FrameGate::default(),
            scroll_gate: FrameGate::default(),
            shortcut_gate: FrameGate::default(),
            opacity_hover: None,
            opacity_keepalive: None,
            opacity_open: false,
            opacity_anchor: Pos2::ZERO,
            transparency_save_at: None,
            brand: ctx.load_texture(
                "miniterm-brand",
                egui::ColorImage::from_rgba_unmultiplied([64, 64], &crate::brand::rgba(64)),
                egui::TextureOptions::LINEAR,
            ),
            settings_gate: FrameGate::default(),
            broker,
            _ipc_server: ipc_server,
            toast_worker,
            notification_preferences,
            notices_open: false,
            diagnostic_ui,
            diagnostic_scroll,
            diagnostic_sample_requested: false,
            diagnostic_settings_keys: Vec::new(),
            diagnostic_sample_ready,
            adapter_name: cc
                .wgpu_render_state
                .as_ref()
                .map(|state| state.adapter.get_info().name)
                .unwrap_or_default(),
        };
        app.load_fields();
        native::position(hwnd, app.last_scale, false, &app.settings.window)?;
        app.appearance
            .apply(&app.display_watch, &app.settings.window)?;
        native::show(hwnd, std::env::args().any(|arg| arg == "--stale-notice"));
        app.start_session(ctx);
        Ok(app)
    }

    fn start_session(&mut self, ctx: &Context) {
        if self.sessions.len() + self.starting >= 4 {
            self.error = Some("최대 4개 세션을 열 수 있습니다.".into());
            return;
        }
        self.starting += 1;
        let profile = self.settings.profile.clone();
        let prompt = self.saved_settings.prompt.clone();
        let scrollback = self.settings.scrollback as usize;
        let tx = self.started_tx.clone();
        let ctx = ctx.clone();
        // Register before spawning Bash: startup files may emit agent events immediately.
        let id = uuid::Uuid::new_v4().to_string();
        let exited = Arc::new(AtomicBool::new(false));
        let light = Arc::new(AtomicBool::new(self.settings.theme == "light"));
        let broker = self.broker.clone();
        {
            let mut broker = broker.lock().unwrap_or_else(|p| p.into_inner());
            broker.register(id.clone());
            broker.sessions.get_mut(&id).unwrap().exited = Some(exited.clone());
        }
        let hwnd = self.hwnd;
        thread::spawn(move || {
            let result = Session::start_with_id(
                profile,
                scrollback,
                ctx.clone(),
                id.clone(),
                exited,
                light,
                prompt,
            );
            if let Ok(session) = &result {
                session
                    .visible
                    .store(native::visible(hwnd), Ordering::Release);
            } else {
                broker.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
            }
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }

    fn remove_session(&mut self, index: usize) {
        let id = self.sessions[index].id.clone();
        crate::codex_link::close_session(&id);
        self.broker
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&id);
        self.sessions.remove(index);
        self.selected = selection_after_removal(self.selected, index, self.sessions.len());
        self.close_confirm = self.close_confirm.and_then(|closing| {
            if closing == index {
                None
            } else {
                Some(closing - usize::from(closing > index))
            }
        });
        if self.pending_activation.as_ref() == Some(&id) {
            self.pending_activation = None;
        }
        self.rows.clear();
        self.composition.clear();
        self.composing = false;
        if self.sessions.is_empty() && self.starting == 0 && self.error.is_none() {
            self.hide();
        }
    }

    fn select_tab(&mut self, index: usize) {
        if index >= self.sessions.len() {
            return;
        }
        self.selected = index;
        self.terminal_focus = true;
        self.composition.clear();
        self.composing = false;
        self.rows.clear();
    }

    fn toggle_mini_view(&mut self) {
        self.settings.window.mini_view = match self.settings.window.mini_view {
            crate::geometry::MiniView::Input => crate::geometry::MiniView::Summary,
            crate::geometry::MiniView::Summary => crate::geometry::MiniView::Input,
        };
        self.saved_settings.window.mini_view = self.settings.window.mini_view;
        if let Err(error) = self.saved_settings.save(&self.settings_path) {
            self.error = Some(error);
        }
        self.rows.clear();
        self.terminal_focus = true;
    }

    fn hud_shortcut(&mut self, ctx: &Context, shortcut: HudShortcut) {
        match shortcut {
            HudShortcut::NewTab => {
                self.start_session(ctx);
                self.terminal_focus = true;
            }
            HudShortcut::CloseTab if !self.sessions.is_empty() => {
                self.close_confirm = Some(self.selected);
                self.expand(true);
            }
            HudShortcut::NextTab | HudShortcut::PreviousTab if !self.sessions.is_empty() => {
                let count = self.sessions.len();
                let index = match shortcut {
                    HudShortcut::NextTab => (self.selected + 1) % count,
                    _ => (self.selected + count - 1) % count,
                };
                self.select_tab(index);
            }
            HudShortcut::SelectTab(index) => self.select_tab(index),
            HudShortcut::Settings => self.open_settings(),
            HudShortcut::ToggleExpanded => {
                self.expand(!self.expanded);
                self.terminal_focus = true;
            }
            HudShortcut::ToggleMiniView => self.toggle_mini_view(),
            HudShortcut::FontSize(delta) => {
                let mut next = self.saved_settings.clone();
                next.font_size = if delta == 0 {
                    Settings::default().font_size
                } else {
                    (next.font_size as i16 + delta).clamp(10, 24) as u16
                };
                if next != self.saved_settings {
                    if let Err(error) = self.save_preferences(ctx, next) {
                        self.error = Some(error);
                    }
                }
            }
            HudShortcut::ScrollLines(_)
            | HudShortcut::ScrollPage(_)
            | HudShortcut::ScrollEnd(_) => {
                self.expand(true);
                if let Some(session) = self.sessions.get(self.selected) {
                    let mut term = session.term.lock();
                    let scroll = match shortcut {
                        HudShortcut::ScrollLines(lines) => Scroll::Delta(lines),
                        HudShortcut::ScrollPage(true) => Scroll::PageUp,
                        HudShortcut::ScrollPage(false) => Scroll::PageDown,
                        HudShortcut::ScrollEnd(true) => Scroll::Top,
                        _ => Scroll::Bottom,
                    };
                    term.scroll_display(scroll);
                    self.rows.clear();
                }
            }
            _ => {}
        }
        ctx.request_repaint();
    }

    fn quick_transparency(&mut self, value: u16, ctx: &Context) {
        let value = value.min(80);
        if self.settings.window.transparency != value {
            self.settings.window.transparency = value;
            self.transparency_save_at =
                Some(Instant::now() + std::time::Duration::from_millis(250));
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn save_quick_transparency(&mut self) {
        if self.transparency_save_at.take().is_none() {
            return;
        }
        let mut saved = self.saved_settings.clone();
        saved.window.transparency = self.settings.window.transparency;
        match saved.save(&self.settings_path) {
            Ok(()) => self.saved_settings = saved,
            Err(error) => self.error = Some(error),
        }
    }

    fn opacity_popup(&mut self, ctx: &Context) {
        if !self.opacity_open {
            return;
        }
        let mut value = self.settings.window.transparency;
        let mut active = false;
        let mut close = false;
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("transparency-popup"),
            egui::ViewportBuilder::default()
                .with_title("투명도")
                .with_inner_size([72.0, 160.0])
                .with_position(self.opacity_anchor)
                .with_decorations(false)
                .with_resizable(false)
                .with_taskbar(false)
                .with_always_on_top()
                .with_active(false),
            |ctx, _| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label("투명도");
                        ui.spacing_mut().slider_width = 103.0;
                        let slider = ui.add(
                            egui::Slider::new(&mut value, 0..=80)
                                .vertical()
                                .show_value(false),
                        );
                        self.record_widget("opacity-slider", slider.rect);
                        active |= slider.dragged();
                        ui.label(format!("{value}%"));
                    });
                    active |= ui.rect_contains_pointer(ui.max_rect());
                });
                close = ctx.input(|i| i.viewport().close_requested() || i.key_pressed(Key::Escape));
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            },
        );
        self.quick_transparency(value, ctx);
        if active {
            self.opacity_keepalive = Some(Instant::now());
        }
        if close
            || self
                .opacity_keepalive
                .is_none_or(|time| time.elapsed().as_millis() > 350)
        {
            self.opacity_open = false;
            self.opacity_hover = None;
            self.terminal_focus = true;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn expand(&mut self, expanded: bool) {
        self.expanded = expanded;
        self.display_watch.request_repaint();
        self.display_watch.set_resize_mode(expanded);
        let bounds =
            native::target_bounds(self.hwnd, self.last_scale, expanded, &self.settings.window)
                .and_then(|to| native::window_bounds(self.hwnd).map(|from| (from, to)));
        match bounds {
            Ok((from, to)) => {
                if expanded {
                    if let Some(session) = self.sessions.get(self.selected) {
                        let font = FontId::monospace(self.settings.font_size as f32);
                        let width = self
                            .display_watch
                            .context()
                            .fonts_mut(|f| f.glyph_width(&font, 'M'))
                            .max(1.0);
                        let height = (font.size * 1.38).ceil();
                        let cols = ((to.width as f64 / self.last_scale - 18.0) / width as f64)
                            .floor()
                            .clamp(2.0, 1000.0) as u16;
                        let rows = ((to.height as f64 / self.last_scale - 35.0) / height as f64)
                            .floor()
                            .clamp(1.0, 500.0) as u16;
                        session.resize(cols, rows, width as u16, height as u16);
                    }
                }
                let anchored = from.x + from.width as i32 == to.x + to.width as i32
                    && from.y + from.height as i32 == to.y + to.height as i32;
                if from != to
                    && anchored
                    && native::visible(self.hwnd)
                    && !self.settings.window.reduced_motion
                    && native::animations_enabled()
                {
                    self.transition = Some(WindowTransition {
                        from,
                        to,
                        started: Instant::now(),
                    });
                    self.display_watch.request_repaint();
                } else {
                    self.transition = None;
                    if let Err(error) = native::set_bounds(self.hwnd, to) {
                        self.error = Some(error);
                    }
                }
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn hide(&mut self) {
        self.save_quick_transparency();
        self.opacity_open = false;
        self.opacity_hover = None;
        self.close_settings();
        self.notices_open = false;
        self.quit_confirm = false;
        self.close_confirm = None;
        for session in &self.sessions {
            session.visible.store(false, Ordering::Release);
        }
        native::hide(self.hwnd);
        self.expand(false);
    }

    fn activate_session(&mut self, id: &str) {
        if let Some(index) = self
            .sessions
            .iter()
            .position(|session| session.id == id && !session.exited.load(Ordering::Acquire))
        {
            self.pending_activation = None;
            self.selected = index;
            self.close_settings();
            self.notices_open = false;
            self.composition.clear();
            self.composing = false;
            self.terminal_focus = true;
            self.broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .read(id);
            self.rows.clear();
        } else {
            let starting = self
                .broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .sessions
                .get(id)
                .is_some_and(|status| {
                    status.alive
                        && !status
                            .exited
                            .as_ref()
                            .is_some_and(|exit| exit.load(Ordering::Acquire))
                });
            if starting {
                self.pending_activation = Some(id.to_owned());
                self.terminal_focus = false;
            } else {
                self.pending_activation = None;
                self.error = Some("이 알림의 세션은 이미 종료되었습니다.".into());
            }
        }
        self.expand(true);
        native::show(self.hwnd, true);
    }

    fn close_settings(&mut self) {
        if self.settings_open {
            let ctx = self.display_watch.context();
            ctx.send_viewport_cmd_to(settings_viewport_id(), egui::ViewportCommand::Close);
            ctx.request_repaint();
            native::wake_hidden(self.hwnd);
        }
        self.settings_open = false;
    }

    fn open_settings(&mut self) {
        self.save_quick_transparency();
        self.opacity_open = false;
        self.opacity_hover = None;
        self.edited_settings = self.settings.clone();
        self.settings_observed = self.saved_settings.clone();
        self.settings_error = None;
        self.integration_feedback = None;
        self.load_fields();
        self.available_monitors = native::monitors().unwrap_or_default();
        self.available_distributions = crate::shell::wsl_distributions();
        if self.distribution.is_empty() {
            if let Some(distro) = self
                .available_distributions
                .iter()
                .find(|distro| distro.version == 2 && !distro.name.starts_with("docker-desktop"))
            {
                self.distribution = distro.name.clone();
            }
        }
        let scale = self.last_scale as f32;
        self.settings_position = self
            .available_monitors
            .iter()
            .find(|monitor| monitor.name == self.settings.window.monitor)
            .or_else(|| {
                self.available_monitors
                    .iter()
                    .find(|monitor| monitor.primary)
            })
            .map(|monitor| {
                Pos2::new(
                    monitor.work.x as f32 / scale
                        + (monitor.work.width as f32 / scale - SETTINGS_SIZE.x).max(0.0) / 2.0,
                    monitor.work.y as f32 / scale
                        + (monitor.work.height as f32 / scale - SETTINGS_SIZE.y).max(0.0) / 2.0,
                )
            });
        self.settings_open = true;
        self.settings_native_ready = false;
        self.terminal_focus = false;
    }

    fn save_preferences(&mut self, ctx: &Context, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        let previous = &self.saved_settings;
        let profile_changed = settings.profile != previous.profile;
        let font_changed = settings.font_family != previous.font_family;
        let shortcut_changed = settings.shortcut != previous.shortcut;
        let prompt_changed = settings.prompt != previous.prompt;
        let theme_changed = settings.theme != previous.theme;
        let placement_changed = window_placement_changed(&previous.window, &settings.window);
        if profile_changed {
            settings.profile.command("validation")?;
        }
        if font_changed && !settings.font_family.trim().is_empty() {
            let path = std::path::Path::new(&settings.font_family);
            let metadata =
                std::fs::metadata(path).map_err(|_| "터미널 글꼴 파일을 찾지 못했습니다.")?;
            if metadata.len() > 20 * 1024 * 1024 {
                return Err("글꼴 파일은 20MiB 이하여야 합니다.".into());
            }
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            ttf_parser::Face::parse(&bytes, 0)
                .map_err(|_| "지원하는 TTF/OTF 글꼴 파일을 지정하세요.")?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if shortcut_changed {
            self.display_watch.register_shortcut(&settings.shortcut)?;
        }
        let start_changed = settings.autostart != self.saved_settings.autostart;
        if start_changed {
            if let Err(error) = native::autostart(&exe, settings.autostart) {
                if shortcut_changed {
                    let _ = self
                        .display_watch
                        .register_shortcut(&self.saved_settings.shortcut);
                }
                return Err(error);
            }
        }
        let persisted = (|| {
            if prompt_changed {
                for session in &self.sessions {
                    session.update_prompt(&settings.prompt)?;
                }
            }
            settings.save(&self.settings_path)
        })();
        if let Err(error) = persisted {
            if prompt_changed {
                for session in &self.sessions {
                    let _ = session.update_prompt(&self.saved_settings.prompt);
                }
            }
            if shortcut_changed {
                let _ = self
                    .display_watch
                    .register_shortcut(&self.saved_settings.shortcut);
            }
            if start_changed {
                let _ = native::autostart(&exe, self.saved_settings.autostart);
            }
            return Err(error);
        }
        if font_changed {
            install_fonts(ctx, &settings.font_family);
        }
        *self
            .notification_preferences
            .lock()
            .unwrap_or_else(|p| p.into_inner()) =
            (settings.notifications, settings.notification_sound);
        self.saved_settings = settings.clone();
        self.settings = settings;
        if placement_changed {
            self.transition = None;
            if let Err(error) = native::position(
                self.hwnd,
                self.last_scale,
                self.expanded,
                &self.settings.window,
            ) {
                self.error = Some(error);
            }
        }
        self.rows.clear();
        if theme_changed {
            ctx.set_visuals(hud_visuals(self.settings.theme == "light"));
            ctx.request_discard("theme changed");
        }
        ctx.request_repaint();
        ctx.request_repaint_of(egui::ViewportId::ROOT);
        Ok(())
    }

    fn load_fields(&mut self) {
        match &self.settings.profile {
            ShellProfile::GitBash {
                bash_path,
                directory,
            } => {
                self.wsl = false;
                self.bash_path = bash_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.bash_directory = directory
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
            }
            ShellProfile::Wsl {
                distribution,
                user,
                directory,
            } => {
                self.wsl = true;
                self.distribution = distribution.clone();
                self.wsl_user = user.clone().unwrap_or_default();
                self.wsl_directory = directory.clone();
            }
        }
    }
    fn set_theme(&mut self, ctx: &Context, theme: &str) {
        if self.settings.theme == theme {
            return;
        }
        let mut settings = self.settings.clone();
        settings.theme = theme.into();
        match self.save_preferences(ctx, settings) {
            Ok(()) => self.edited_settings.theme = theme.into(),
            Err(error) => self.settings_error = Some(error),
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let frame = ctx.cumulative_frame_nr();
        let mut expand = false;
        let compact = ui.available_width() < 420.0;
        let action_size = if compact { 20.0 } else { 22.0 };
        ui.horizontal(|ui| {
            if compact {
                ui.spacing_mut().item_spacing.x = 3.0;
            }
            let title = ui
                .add(
                    egui::Image::new(&self.brand)
                        .fit_to_exact_size(Vec2::splat(action_size))
                        .sense(Sense::click_and_drag()),
                )
                .on_hover_text("MiniTerm · 드래그: 창 이동 · 더블 클릭 / Ctrl+Alt+Enter: 확장/축소");
            self.record_widget("header-brand", title.rect);
            if title.drag_started() {
                self.transition = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if title.is_pointer_button_down_on() {
                self.terminal_focus = true;
            }
            for (index, session) in self.sessions.iter().enumerate() {
                let agent_status = self
                    .broker
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .sessions
                    .get(&session.id)
                    .cloned();
                let mut label = egui::RichText::new((index + 1).to_string());
                if session.exited.load(Ordering::Acquire) {
                    label = label.weak().strikethrough();
                }
                let tab = ui
                    .selectable_label(index == self.selected, label)
                    .on_hover_text(format!("{}\nCtrl+Alt+{}: 이 탭 선택\nCtrl+Tab / Ctrl+Shift+Tab: 탭 이동\n오른쪽 클릭 / Ctrl+Shift+W: 세션 종료", session.label, index + 1));
                self.record_widget(&format!("tab-{index}"), tab.rect);
                if index == self.selected {
                    self.record_widget("selected-tab", tab.rect);
                }
                if agent_status.as_ref().is_some_and(|status| status.unread) {
                    ui.painter().circle_filled(
                        tab.rect.right_top() + Vec2::new(-2.0, 2.0),
                        2.2,
                        Color32::from_rgb(214, 66, 83),
                    );
                }
                if tab.clicked() && self.header_gate.claim(frame) {
                    self.selected = index;
                    self.terminal_focus = true;
                    self.composition.clear();
                    self.composing = false;
                    self.rows.clear();
                }
                if tab.secondary_clicked() && self.header_gate.claim(frame) {
                    self.close_confirm = Some(index);
                }
            }
            let selected_status = self.sessions.get(self.selected).and_then(|session| {
                self.broker
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .sessions
                    .get(&session.id)
                    .cloned()
            });
            let status = if self.starting > 0 {
                "Bash 연결 중".to_string()
            } else if self.sessions.is_empty() {
                "연결 필요".to_string()
            } else if self.sessions[self.selected].exited.load(Ordering::Acquire) {
                "셸 종료".to_string()
            } else {
                let session = &self.sessions[self.selected];
                let title = session
                    .title
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                if self.settings.window.mini_show_title && !title.is_empty() {
                    title
                } else {
                    selected_status
                        .and_then(|status| {
                            if self.settings.window.mini_show_title {
                                status.progress.record().map(|record| record.report.title)
                            } else {
                                None
                            }
                            .or_else(|| status.agent.map(|agent| agent.label().to_owned()))
                        })
                        .unwrap_or_default()
                }
            };
            let status_width =
                (ui.available_width() - 3.0 * (action_size + ui.spacing().item_spacing.x)).max(0.0);
            let state = ui.add_sized(
                [
                    if status.is_empty() { 0.0 } else { status_width },
                    action_size,
                ],
                egui::Label::new(egui::RichText::new(&status).small().color(
                    if self.settings.theme == "light" {
                        Color32::from_rgb(73, 84, 98)
                    } else {
                        Color32::from_rgb(165, 178, 196)
                    },
                ))
                .truncate()
                .selectable(false)
                .sense(Sense::click_and_drag()),
            );
            if !status.is_empty() {
                self.record_widget("session-title", state.rect);
            }
            if state.drag_started() {
                self.transition = None;
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            if state.is_pointer_button_down_on() {
                self.terminal_focus = true;
            }
            let drag_space = ui.available_rect_before_wrap();
            let actions = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let hide = ui.add(
                    IconButton::new(
                        ActionIcon::Hide,
                        "클릭: 숨기기 · 잠시 올려두기: 투명도 · Ctrl+Alt+휠: 투명도 조절",
                    )
                    .size(action_size),
                );
                self.record_widget("header-hide", hide.rect);
                if hide.hovered()
                    && !self.settings_open
                    && !self.quit_confirm
                    && self.close_confirm.is_none()
                    && self.error.is_none()
                {
                    let start = self.opacity_hover.get_or_insert_with(Instant::now);
                    self.opacity_keepalive = Some(Instant::now());
                    if start.elapsed().as_millis() >= 450 {
                        self.opacity_open = true;
                        if let Ok(bounds) = native::window_bounds(self.hwnd) {
                            self.opacity_anchor = Pos2::new(
                                bounds.x as f32 / self.last_scale as f32 + hide.rect.center().x
                                    - 36.0,
                                bounds.y as f32 / self.last_scale as f32 + hide.rect.top() - 166.0,
                            );
                        }
                    } else {
                        ctx.request_repaint_after(std::time::Duration::from_millis(450));
                    }
                } else if !self.opacity_open {
                    self.opacity_hover = None;
                }
                if hide.clicked() && self.header_gate.claim(frame) {
                    self.hide();
                }
                let layout = ui.add(
                    IconButton::new(
                        if self.settings.window.mini_view == crate::geometry::MiniView::Summary {
                            ActionIcon::Summary
                        } else {
                            ActionIcon::Terminal
                        },
                        if self.settings.window.mini_view == crate::geometry::MiniView::Summary {
                            "미니 보기: 작업 요약 · 클릭 / Ctrl+Alt+M: 입력줄로 전환"
                        } else {
                            "미니 보기: 입력줄 · 클릭 / Ctrl+Alt+M: 작업 요약으로 전환"
                        },
                    )
                    .selected(self.settings.window.mini_view == crate::geometry::MiniView::Summary)
                    .size(action_size),
                );
                self.record_widget("header-layout", layout.rect);
                if layout.clicked() && self.header_gate.claim(frame) {
                    self.toggle_mini_view();
                }
                let add = ui.add_enabled(
                    self.sessions.len() + self.starting < 4,
                    IconButton::new(ActionIcon::Add, "새 탭 · Ctrl+Shift+T · 최대 4개")
                        .size(action_size),
                );
                self.record_widget("header-add", add.rect);
                if add.clicked() && self.header_gate.claim(frame) {
                    self.start_session(ctx);
                    self.terminal_focus = true;
                }
                ui.min_rect().left()
            });
            let gap = Rect::from_min_max(
                drag_space.min,
                egui::pos2(actions.inner, drag_space.bottom()),
            );
            if self.settings_open
                || self.notices_open
                || self.quit_confirm
                || self.close_confirm.is_some()
                || self.error.is_some()
            {
                self.display_watch.set_header_regions(&[], self.last_scale);
            } else {
                self.display_watch
                    .set_header_regions(&[title.rect, state.rect, gap], self.last_scale);
            }
            if gap.is_positive() {
                let drag = ui
                    .interact(gap, ui.id().with("window-drag"), Sense::click_and_drag())
                    .on_hover_text("드래그: 창 이동 · 더블 클릭: 확장/축소")
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if drag.drag_started() {
                    self.transition = None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag.double_clicked() && self.header_gate.claim(frame) {
                    expand = true;
                }
            }
        });
        if expand {
            self.expand(!self.expanded);
        }
        if self.close_confirm.is_some() && !self.expanded {
            self.expand(true);
        }
    }

    fn paint_summary(
        &self,
        ui: &mut egui::Ui,
        snapshot: &crate::mini::Summary,
        rect: Rect,
        font: &FontId,
        height: f32,
        cell_width: f32,
    ) -> Rect {
        let preferences = &self.settings.window;
        let battery = preferences.mini_show_battery && snapshot.battery_row.is_some();
        let battery_rect = Rect::from_min_max(
            Pos2::new(rect.left(), (rect.bottom() - height).max(rect.top())),
            rect.max,
        );
        let budget = (rect.height() / height).floor() as usize;
        let budget = budget.saturating_sub(usize::from(battery));
        let weak = ui.visuals().weak_text_color();
        let text_color = ui.visuals().text_color();
        let green = if self.settings.theme == "light" {
            Color32::from_rgb(24, 114, 73)
        } else {
            Color32::from_rgb(110, 205, 154)
        };
        let plan = snapshot.plan.as_ref();
        let tooltip = snapshot
            .reported
            .as_ref()
            .map(|record| record.description())
            .or_else(|| plan.map(|plan| plan.description()))
            .unwrap_or_default();
        let mut lines: Vec<(&str, String, Color32)> = Vec::new();
        if preferences.mini_show_status {
            let mut status = snapshot
                .reported
                .as_ref()
                .filter(|record| record.report.state != crate::progress::WorkState::Running)
                .map(|record| record.report.state.label().to_owned())
                .or_else(|| {
                    snapshot
                        .activity
                        .as_deref()
                        .map(crate::mini::activity_label)
                })
                .or_else(|| {
                    snapshot
                        .reported
                        .as_ref()
                        .map(|record| record.report.state.label().to_owned())
                })
                .unwrap_or_else(|| {
                    self.sessions
                        .get(self.selected)
                        .and_then(|session| {
                            self.broker
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .sessions
                                .get(&session.id)
                                .filter(|status| {
                                    status.agent.is_some()
                                        && status.state != crate::agent::State::Unknown
                                })
                                .map(|status| status.state.label().to_owned())
                        })
                        .unwrap_or_default()
                });
            if let Some(plan) = plan {
                let activity = status;
                status = String::new();
                if let Some(percent) = plan.percent() {
                    status.push_str(&format!(
                        "{percent}% · {}/{} 완료",
                        plan.completed(),
                        plan.tasks.len()
                    ));
                } else {
                    status.push_str("계획 · 일부 표시");
                }
                if !activity.is_empty() {
                    status.push_str("  ·  ");
                    status.push_str(&activity);
                }
            }
            if !status.is_empty() {
                lines.push(("summary-status", status, text_color));
            }
        }
        if preferences.mini_show_tasks {
            if let Some(plan) = plan {
                let paused = snapshot.reported.as_ref().is_some_and(|record| {
                    matches!(
                        record.report.state,
                        crate::progress::WorkState::Paused | crate::progress::WorkState::Failed
                    )
                });
                let current = if paused {
                    plan.tasks
                        .iter()
                        .rev()
                        .find(|task| {
                            matches!(
                                task.state,
                                crate::mini::TaskState::Running | crate::mini::TaskState::Stopped
                            )
                        })
                        .or_else(|| plan.current())
                } else {
                    plan.current()
                };
                if let Some(task) = current {
                    let label = if snapshot.activity.as_deref().is_some_and(|text| {
                        text.starts_with("Interrupted")
                            || text.starts_with("Goal paused")
                            || text.starts_with("Goal stalled")
                    }) || paused
                        || task.state == crate::mini::TaskState::Stopped
                    {
                        "멈춘 작업"
                    } else if snapshot.reported.as_ref().is_some_and(|record| {
                        record.report.state != crate::progress::WorkState::Running
                    }) {
                        "마지막 작업"
                    } else if task.state == crate::mini::TaskState::Running {
                        "진행"
                    } else {
                        "다음"
                    };
                    lines.push((
                        "summary-task",
                        format!("{label}: {}", task.text),
                        text_color,
                    ));
                }
                if let Some(task) = plan.tasks.iter().rev().find(|task| {
                    task.state == crate::mini::TaskState::Stopped && current != Some(*task)
                }) {
                    lines.push(("summary-stopped", format!("중단: {}", task.text), weak));
                }
                if let Some(task) = plan.last_completed() {
                    lines.push(("summary-completed", format!("완료: {}", task.text), green));
                }
            }
        }
        if plan.is_none() {
            if let Some(text) = snapshot
                .operation
                .as_ref()
                .filter(|_| preferences.mini_show_output)
            {
                lines.push(("summary-output", text.clone(), weak));
            } else if preferences.mini_show_tasks {
                lines.push(("summary-unreported", "작업 계획 없음".into(), weak));
            }
        }
        // One available line still includes the current task; hover retains the
        // complete observed list when mouse resizing reduces the visible budget.
        if budget == 1 && lines.len() > 1 && lines[0].0 == "summary-status" {
            lines[0].1 = format!("{}  ·  {}", lines[0].1, lines[1].1);
        }
        for (index, (name, text, color)) in lines.into_iter().take(budget).enumerate() {
            let line_rect = Rect::from_min_size(
                rect.min + Vec2::new(0.0, index as f32 * height),
                Vec2::new(rect.width(), height),
            );
            let hint = if tooltip.is_empty() {
                text.clone()
            } else {
                format!("{text}\n\n{tooltip}")
            };
            summary_label(
                ui,
                line_rect,
                egui::RichText::new(&text).font(font.clone()).color(color),
                &hint,
                Vec2::new(
                    (rect.width() - 20.0).clamp(120.0, 480.0),
                    (rect.height() - 12.0).max(24.0),
                ),
                name,
            );
            self.record_widget(name, line_rect);
            if name == "summary-status" {
                if let Some(percent) = plan.and_then(|plan| plan.percent()) {
                    let track = Rect::from_min_size(
                        line_rect.left_bottom() - Vec2::new(0.0, 1.5),
                        Vec2::new(rect.width(), 1.5),
                    );
                    ui.painter()
                        .rect_filled(track, 1.0, green.gamma_multiply(0.16));
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            track.min,
                            Vec2::new(track.width() * percent as f32 / 100.0, track.height()),
                        ),
                        1.0,
                        green,
                    );
                    self.record_widget("summary-progress", track);
                    self.record_widget(
                        "summary-progress-fill",
                        Rect::from_min_size(
                            track.min,
                            Vec2::new(track.width() * percent as f32 / 100.0, track.height()),
                        ),
                    );
                }
            }
        }
        if battery {
            self.record_widget("summary-battery", battery_rect);
            let response = ui.interact(
                battery_rect,
                ui.id().with("summary-battery"),
                Sense::hover(),
            );
            if let Some(text) = snapshot.battery_text.as_deref() {
                if snapshot
                    .battery_columns
                    .is_some_and(|columns| columns as f32 * cell_width > battery_rect.width())
                {
                    response.on_hover_text(text);
                }
            }
            battery_rect
        } else {
            rect
        }
    }

    fn terminal(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        let frame = ctx.cumulative_frame_nr();
        if !self.expanded
            && self.settings.window.mini_view == crate::geometry::MiniView::Summary
            && native::focused(self.hwnd)
            && self.terminal_focus
            && !self.settings_open
            && ctx.input(|i| {
                i.events.iter().any(|e| {
                    matches!(
                        e,
                        Event::Text(_)
                            | Event::Paste(_)
                            | Event::Ime(ImeEvent::Preedit(_) | ImeEvent::Commit(_))
                            | Event::Key { pressed: true, .. }
                    )
                })
            })
        {
            self.expand(true);
        }
        let mini = !self.expanded;
        let summary = mini && self.settings.window.mini_view == crate::geometry::MiniView::Summary;
        let Some(session) = self.sessions.get(self.selected) else {
            ui.centered_and_justified(|ui| {
                ui.label(if self.starting > 0 {
                    "셸에 연결하고 있습니다…"
                } else {
                    "+ 또는 Ctrl+Shift+T로 새 탭을 여세요. 트레이 메뉴에서 셸을 선택할 수 있습니다."
                });
            });
            return;
        };
        session.repaint_pending.store(false, Ordering::Release);
        let font = FontId::monospace(self.settings.font_size as f32);
        let width = ui.fonts_mut(|fonts| fonts.glyph_width(&font, 'M')).max(1.0);
        let height = (self.settings.font_size as f32 * if summary { 1.30 } else { 1.38 }).ceil();
        let reference = ui
            .painter()
            .layout_no_wrap("M".into(), font.clone(), Color32::WHITE);
        // egui rebuilds the atlas when text coverage changes (dark/light), fonts change
        // or the atlas fills. Cached galleys contain UVs from that atlas. A reference
        // layout used every frame retains its Arc until egui recreates its font cache.
        if !self
            .atlas_reference
            .as_ref()
            .is_some_and(|old| Arc::ptr_eq(old, &reference))
        {
            self.rows.clear();
        }
        self.atlas_reference = Some(reference.clone());
        let baseline = (height - reference.size().y).max(0.0) * 0.5 + galley_baseline(&reference);
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.record_widget("terminal", rect);
        self.display_watch.set_body_region(
            (!self.settings_open
                && !self.notices_open
                && !self.quit_confirm
                && self.close_confirm.is_none()
                && self.error.is_none())
            .then_some(rect),
            self.last_scale,
        );
        if response.is_pointer_button_down_on() {
            self.terminal_focus = true;
            response.request_focus();
        }
        let cols = (rect.width() / width).floor().clamp(2.0, 1000.0) as u16;
        let rows = (rect.height() / height).floor().clamp(1.0, 500.0) as u16;
        if self.expanded && self.transition.is_none() && !self.display_watch.moving() {
            session.resize(cols, rows, width as u16, height as u16);
        }
        let input_events = ctx.input(|input| input.events.clone());
        let focused = native::focused(self.hwnd)
            && self.terminal_focus
            && !self.settings_open
            && !self.quit_confirm
            && self.close_confirm.is_none();
        if let Some(trace) = &self.diagnostic_scroll {
            let mut trace = trace.lock().unwrap_or_else(|p| p.into_inner());
            trace.focused = focused;
            trace.composing = self.composing;
            trace.enter_events += input_events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        Event::Key {
                            key: Key::Enter,
                            pressed: true,
                            ..
                        }
                    )
                })
                .count();
        }
        if focused {
            response.request_focus();
        }
        let summary_snapshot = summary.then(|| {
            let report = self
                .broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .sessions
                .get(&session.id)
                .and_then(|status| status.progress.record());
            session.summary().with_report(report)
        });
        let render_rect = summary_snapshot
            .as_ref()
            .map(|snapshot| self.paint_summary(ui, snapshot, rect, &font, height, width))
            .unwrap_or(rect);
        let mut term = session.term.lock();
        let cols = cols.min(term.columns() as u16);
        let offset = term.grid().display_offset();
        let mut visible_rows: Vec<i32> = if let Some(snapshot) = &summary_snapshot {
            if self.settings.window.mini_show_battery {
                snapshot.battery_row.into_iter().collect()
            } else {
                Vec::new()
            }
        } else if mini {
            crate::mini::preview_rows(
                &term,
                if !self.settings.window.mini_show_output {
                    usize::from(!summary)
                } else {
                    rows as usize
                },
                !summary,
            )
        } else {
            let rows = rows.min(term.screen_lines() as u16) as i32;
            let start = if self.transition.is_some() {
                term.screen_lines() as i32 - rows
            } else {
                0
            };
            (start..start + rows).map(|r| r - offset as i32).collect()
        };
        let render_rect = if mini && !summary {
            Rect::from_min_max(
                Pos2::new(
                    rect.left(),
                    rect.bottom() - (visible_rows.len() as f32 * height).min(rect.height()),
                ),
                rect.right_bottom(),
            )
        } else {
            render_rect
        };
        self.record_widget("terminal-content", render_rect);
        // Isolated GUI tests observe only the bounds of the known local Codex
        // chooser. No transcript text is exposed through the diagnostic IPC.
        if self.diagnostic_ui.is_some() && !mini && offset == 0 {
            let mut goal_top = None;
            let mut goal_bottom = None;
            for (index, row) in visible_rows.iter().enumerate() {
                let text: String = (0..cols as usize)
                    .map(|col| term.grid()[Line(*row)][Column(col)].c)
                    .collect();
                if text.contains("Resume paused goal?") {
                    goal_top = Some(index);
                }
                if goal_top.is_some() && text.contains("enter select") && text.contains("esc back")
                {
                    goal_bottom = Some(index);
                }
            }
            if let (Some(top), Some(bottom)) = (goal_top, goal_bottom) {
                self.record_widget(
                    "fixture-goal-choice",
                    Rect::from_min_max(
                        rect.min + Vec2::new(0.0, top as f32 * height),
                        Pos2::new(rect.right(), rect.top() + (bottom + 1) as f32 * height),
                    ),
                );
            }
        }
        let mut column_start = if mini && !summary && offset == 0 {
            crate::mini::horizontal_start(
                term.grid().cursor.point.column.0,
                cols as usize,
                term.columns(),
            )
        } else {
            0
        };
        if mini
            && column_start > 0
            && term.grid()[term.grid().cursor.point.line][Column(column_start)]
                .flags
                .contains(Flags::WIDE_CHAR_SPACER)
        {
            column_start -= 1;
        }
        let rows = visible_rows.len();
        let point_at = |pos: Pos2| {
            Point::new(
                Line(
                    visible_rows
                        .get(
                            ((pos.y - render_rect.top()) / height)
                                .floor()
                                .clamp(0.0, rows.saturating_sub(1) as f32)
                                as usize,
                        )
                        .copied()
                        .unwrap_or(0),
                ),
                Column(
                    ((pos.x - rect.left()) / width)
                        .floor()
                        .clamp(0.0, cols as f32 - 1.0) as usize
                        + column_start,
                ),
            )
        };
        let shift = ctx.input(|input| input.modifiers.shift);
        let wheel_delta = ctx.input(|input| input.raw_scroll_delta.y);
        let mut typed_input = false;
        let mouse_program = term.mode().intersects(TermMode::MOUSE_MODE) && !shift && !summary;
        let body_expansion = response.double_clicked() && !shift && self.header_gate.claim(frame);
        if response.double_clicked() && !mouse_program && !body_expansion && !summary {
            if let Some(pos) = response.interact_pointer_pos() {
                term.selection = Some(Selection::new(
                    SelectionType::Semantic,
                    point_at(pos),
                    Side::Left,
                ));
            }
        } else if response.drag_started() && !mouse_program && !summary {
            if let Some(pos) = ctx.input(|i| i.pointer.press_origin()) {
                term.selection = Some(Selection::new(
                    SelectionType::Simple,
                    point_at(pos),
                    Side::Left,
                ));
            }
        } else if response.clicked() && !mouse_program {
            term.selection = None;
        }
        if response.dragged() && !mouse_program && !summary {
            if let (Some(selection), Some(pos)) =
                (term.selection.as_mut(), response.interact_pointer_pos())
            {
                selection.update(point_at(pos), Side::Right);
            }
        }
        if response.hovered() && !summary {
            let scroll = ctx.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > 0.0
                && !ctx.input(|input| opacity_modifiers(input.modifiers))
                && self.scroll_gate.claim(frame)
            {
                if mouse_program {
                    if let Some(pos) = response.hover_pos() {
                        let p = point_at(pos);
                        send_mouse(
                            session,
                            *term.mode(),
                            if scroll > 0.0 { 64 } else { 65 },
                            p.column.0 + 1,
                            (p.line.0 + offset as i32 + 1) as usize,
                            true,
                        );
                    }
                } else if let Some(data) = alternate_wheel_bytes(
                    *term.mode(),
                    shift,
                    self.settings.alternate_scroll,
                    scroll,
                ) {
                    let _ = session.input.send(data);
                } else {
                    term.scroll_display(Scroll::Delta((scroll / height).round() as i32));
                    if term.grid().display_offset() != offset {
                        ctx.request_repaint();
                    }
                }
            }
        }
        if focused && !input_events.is_empty() && self.input_gate.claim(frame) {
            let mut input_batch = Vec::new();
            let win32 = session.input.win32_mode();
            let mut submitted = false;
            for event in input_events {
                let is_text = matches!(&event, Event::Text(_) | Event::Ime(ImeEvent::Commit(_)));
                let is_enter = matches!(
                    &event,
                    Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        ..
                    }
                );
                if let Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    modifiers,
                    ..
                } = &event
                {
                    if let Some(trace) = &self.diagnostic_scroll {
                        trace
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .last_enter_modifiers =
                            [modifiers.ctrl, modifiers.alt, modifiers.shift];
                    }
                }
                submitted |= matches!(&event, Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !win32 || !modifiers.shift)
                    || matches!(&event, Event::Key { key: Key::C, pressed: true, modifiers, .. } if modifiers.ctrl);
                let data = match event {
                    Event::Ime(ImeEvent::Preedit(text)) => {
                        self.composing = !text.is_empty();
                        self.composition = text;
                        None
                    }
                    Event::Ime(ImeEvent::Commit(text)) => {
                        self.composing = false;
                        self.composition.clear();
                        Some(if win32 {
                            crate::win32_input::text(&text, egui::Modifiers::NONE)
                        } else {
                            text.into_bytes()
                        })
                    }
                    Event::Ime(ImeEvent::Disabled) => {
                        self.composing = false;
                        self.composition.clear();
                        None
                    }
                    Event::Text(text) if !self.composing => {
                        let alt = ctx.input(|i| i.modifiers.alt);
                        Some(if win32 {
                            crate::win32_input::text(&text, ctx.input(|i| i.modifiers))
                        } else if alt {
                            format!("\u{1b}{text}").into_bytes()
                        } else {
                            text.into_bytes()
                        })
                    }
                    Event::Paste(text) => Some(paste_bytes(
                        &text,
                        term.mode().contains(TermMode::BRACKETED_PASTE),
                    )),
                    Event::Copy if shift || term.selection_to_string().is_some() => {
                        if let Some(text) = term.selection_to_string() {
                            ctx.copy_text(text);
                        }
                        None
                    }
                    Event::Copy => {
                        submitted = true;
                        Some(if win32 {
                            crate::win32_input::key(Key::C, egui::Modifiers::CTRL).unwrap()
                        } else {
                            vec![3]
                        })
                    }
                    Event::Cut => Some(if win32 {
                        crate::win32_input::key(Key::X, egui::Modifiers::CTRL).unwrap()
                    } else {
                        vec![24]
                    }),
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if !self.composing => {
                        if win32 {
                            crate::win32_input::key(key, modifiers)
                        } else {
                            key_bytes(key, modifiers, *term.mode())
                        }
                    }
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        ..
                    } if mouse_program
                        && !body_expansion
                        && !summary
                        && (rect.contains(pos) || (!pressed && response.drag_stopped())) =>
                    {
                        let point = point_at(pos);
                        send_mouse(
                            session,
                            *term.mode(),
                            0,
                            point.column.0 + 1,
                            (point.line.0 + offset as i32 + 1) as usize,
                            pressed,
                        );
                        None
                    }
                    Event::PointerMoved(pos)
                        if mouse_program
                            && !body_expansion
                            && !summary
                            && rect.contains(pos)
                            && (term.mode().contains(TermMode::MOUSE_MOTION)
                                || (term.mode().contains(TermMode::MOUSE_DRAG)
                                    && response.dragged())) =>
                    {
                        let point = point_at(pos);
                        send_mouse(
                            session,
                            *term.mode(),
                            if ctx.input(|i| i.pointer.primary_down()) {
                                32
                            } else {
                                35
                            },
                            point.column.0 + 1,
                            (point.line.0 + offset as i32 + 1) as usize,
                            true,
                        );
                        None
                    }
                    _ => None,
                };
                if let Some(data) = data {
                    if is_enter {
                        if let Some(trace) = &self.diagnostic_scroll {
                            let mut trace = trace.lock().unwrap_or_else(|p| p.into_inner());
                            trace.enter_encoded += 1;
                            trace.last_enter_win32 = win32;
                            trace.last_enter_frame = frame;
                        }
                    }
                    if is_text {
                        if let Some(trace) = &self.diagnostic_scroll {
                            trace
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .last_text_frame = frame;
                        }
                    }
                    input_batch.extend(data);
                }
            }
            if !input_batch.is_empty() {
                typed_input = true;
                term.scroll_display(Scroll::Bottom);
                let submitted = submitted
                    || (!win32 && (input_batch.contains(&b'\r') || input_batch.contains(&3)));
                if let Err(error) = session.input.send(input_batch) {
                    self.error = Some(error);
                } else if submitted {
                    if let Some(status) = self
                        .broker
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .sessions
                        .get_mut(&session.id)
                    {
                        status.progress.invalidate();
                    }
                    ctx.request_repaint();
                }
            }
        }
        // Wheel and typed input can both change the viewport above. Paint the
        // resulting rows in this frame, even when the CLI is idle and emits no
        // further output to trigger a repaint.
        let paint_offset = term.grid().display_offset();
        if paint_offset != offset && !summary {
            visible_rows = if mini {
                crate::mini::preview_rows(
                    &term,
                    if self.settings.window.mini_show_output {
                        rows
                    } else {
                        1
                    },
                    true,
                )
            } else {
                (0..rows as i32)
                    .map(|row| row - paint_offset as i32)
                    .collect()
            };
            column_start = if mini && paint_offset == 0 {
                crate::mini::horizontal_start(
                    term.grid().cursor.point.column.0,
                    cols as usize,
                    term.columns(),
                )
            } else {
                0
            };
            if column_start > 0
                && term.grid()[term.grid().cursor.point.line][Column(column_start)]
                    .flags
                    .contains(Flags::WIDE_CHAR_SPACER)
            {
                column_start -= 1;
            }
        }
        if paint_offset > 0 {
            self.record_widget("terminal-scrolled", render_rect);
        }
        if let Some(state) = &self.diagnostic_scroll {
            let mut state = state.lock().unwrap_or_else(|p| p.into_inner());
            state.win32_input = session.input.win32_mode();
            state.alternate_screen = term.mode().contains(TermMode::ALT_SCREEN);
            state.mouse_reporting = term.mode().intersects(TermMode::MOUSE_MODE);
            state.history_lines = term.grid().history_size();
            state.display_offset = paint_offset;
            state.codex_ready = false;
            state.codex_writer_blocked = false;
            for row in 0..term.screen_lines() {
                let line = (0..term.columns())
                    .map(|column| term.grid()[Line(row as i32)][Column(column)].c)
                    .collect::<String>();
                state.codex_ready |= line.contains("OpenAI Codex");
                state.codex_writer_blocked |=
                    line.contains("This conversation is open in another app");
            }
            if wheel_delta != 0.0 {
                state.wheel_events += 1;
                state.last_wheel_delta = wheel_delta;
                state.last_wheel_hovered = response.hovered();
                state.last_wheel_typed = typed_input;
            }
        }
        let rect = render_rect;
        let layout_key = (
            cols as usize,
            width.to_bits(),
            height.to_bits(),
            self.settings.theme == "light",
            self.selected,
            rect.min.x.to_bits(),
            rect.min.y.to_bits(),
        );
        if layout_key != self.layout_key {
            self.rows.clear();
            self.layout_key = layout_key;
        }
        self.rows.resize_with(rows, RowCache::default);
        let content = term.renderable_content();
        let colors = *content.colors;
        let selection = content.selection;
        let cursor = content.cursor;
        let mut cells_by_row = vec![Vec::new(); rows];
        for (index, line) in visible_rows.iter().enumerate() {
            for col in column_start..column_start + cols as usize {
                let point = Point::new(Line(*line), Column(col));
                cells_by_row[index].push((point, term.grid()[point].clone()));
            }
        }
        // Release the VT lock before font layout and GPU shape construction.
        drop(term);
        let painter = ui.painter().with_clip_rect(rect);
        let light = self.settings.theme == "light";
        for (row_index, cells) in cells_by_row.into_iter().enumerate() {
            let mut hash = DefaultHasher::new();
            column_start.hash(&mut hash);
            for (point, cell) in &cells {
                cell.c.hash(&mut hash);
                cell.flags.bits().hash(&mut hash);
                cell.zerowidth().hash(&mut hash);
                let (fg, bg) = resolve_cell_colors(cell.fg, cell.bg, cell.flags, &colors, light);
                fg.to_array().hash(&mut hash);
                bg.to_array().hash(&mut hash);
                selection
                    .is_some_and(|selection| selection.contains(*point))
                    .hash(&mut hash);
            }
            let key = hash.finish();
            let cache = &mut self.rows[row_index];
            if cache.hash != key {
                cache.hash = key;
                cache.shapes.clear();
                // A wide glyph spans its leading cell and the next spacer cell.
                // Paint every background first so the spacer cannot cover its right half.
                for (point, cell) in &cells {
                    let pos = rect.min
                        + Vec2::new(
                            (point.column.0 - column_start) as f32 * width,
                            row_index as f32 * height,
                        );
                    let (_, mut bg) =
                        resolve_cell_colors(cell.fg, cell.bg, cell.flags, &colors, light);
                    if selection.is_some_and(|selection| selection.contains(*point)) {
                        bg = Color32::from_rgb(47, 85, 92);
                    }
                    if bg != background(light) {
                        cache.shapes.push(Shape::rect_filled(
                            Rect::from_min_size(pos, Vec2::new(width, height)),
                            0.0,
                            bg,
                        ));
                    }
                }
                for (point, cell) in cells {
                    let pos = rect.min
                        + Vec2::new(
                            (point.column.0 - column_start) as f32 * width,
                            row_index as f32 * height,
                        );
                    let cell_rect = Rect::from_min_size(pos, Vec2::new(width, height));
                    let (mut fg, _) =
                        resolve_cell_colors(cell.fg, cell.bg, cell.flags, &colors, light);
                    if cell.flags.contains(Flags::DIM) {
                        fg = fg.gamma_multiply(0.66);
                    }
                    if selection.is_some_and(|selection| selection.contains(point)) {
                        fg = Color32::WHITE;
                    }
                    if !cell.flags.intersects(
                        Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER | Flags::HIDDEN,
                    ) && (cell.c != ' ' || cell.zerowidth().is_some())
                    {
                        let mut text = cell.c.to_string();
                        if let Some(extra) = cell.zerowidth() {
                            text.extend(extra);
                        }
                        let cell_font = if cell.flags.contains(Flags::BOLD_ITALIC) {
                            FontId::new(font.size, FontFamily::Name("terminal-bolditalic".into()))
                        } else if cell.flags.contains(Flags::BOLD) {
                            FontId::new(font.size, FontFamily::Name("terminal-bold".into()))
                        } else if cell.flags.contains(Flags::ITALIC) {
                            FontId::new(font.size, FontFamily::Name("terminal-italic".into()))
                        } else {
                            font.clone()
                        };
                        let mut galley = painter.layout_no_wrap(text, cell_font, fg);
                        if (!self.settings.font_family.is_empty() || !cell.c.is_ascii())
                            && cell.flags.contains(Flags::ITALIC)
                        {
                            let galley = Arc::make_mut(&mut galley);
                            let mut bounds = Rect::NOTHING;
                            for row in &mut galley.rows {
                                let data = Arc::make_mut(&mut row.row);
                                let ascent = data.glyphs.first().map_or(0.0, |g| g.pos.y);
                                for vertex in &mut data.visuals.mesh.vertices {
                                    vertex.pos.x += (ascent - vertex.pos.y) * 0.18;
                                }
                                data.visuals.mesh_bounds = data.visuals.mesh.calc_bounds();
                                bounds |= data.visuals.mesh_bounds.translate(row.pos.to_vec2());
                            }
                            galley.mesh_bounds = bounds;
                        }
                        let text_pos = terminal_text_pos(pos, &galley, baseline);
                        if !self.settings.font_family.is_empty() && cell.flags.contains(Flags::BOLD)
                        {
                            cache.shapes.push(Shape::galley(
                                text_pos + Vec2::new(0.4 / ctx.pixels_per_point(), 0.0),
                                galley.clone(),
                                fg,
                            ));
                        }
                        cache.shapes.push(Shape::galley(text_pos, galley, fg));
                    }
                    if cell.flags.intersects(Flags::ALL_UNDERLINES) {
                        cache.shapes.push(Shape::line_segment(
                            [
                                cell_rect.left_bottom() - Vec2::new(0.0, 2.0),
                                cell_rect.right_bottom() - Vec2::new(0.0, 2.0),
                            ],
                            Stroke::new(1.0, fg),
                        ));
                    }
                    if cell.flags.contains(Flags::STRIKEOUT) {
                        cache.shapes.push(Shape::line_segment(
                            [cell_rect.left_center(), cell_rect.right_center()],
                            Stroke::new(1.0, fg),
                        ));
                    }
                }
            }
            painter.extend(cache.shapes.clone());
        }
        let cursor_pos = rect.min
            + Vec2::new(
                (cursor.point.column.0 as f32 - column_start as f32) * width,
                visible_rows
                    .iter()
                    .position(|r| *r == cursor.point.line.0)
                    .map_or(-1.0, |r| r as f32)
                    * height,
            );
        let cursor_rect = Rect::from_min_size(cursor_pos, Vec2::new(width, height));
        if !summary && cursor.shape != CursorShape::Hidden && rect.contains(cursor_pos) {
            self.record_widget("terminal-cursor", cursor_rect);
            let color = resolve_color(Color::Named(NamedColor::Cursor), &colors, light);
            match cursor.shape {
                CursorShape::Beam if focused => {
                    painter.rect_filled(
                        Rect::from_min_size(cursor_pos, Vec2::new(2.0, height)),
                        0.0,
                        color,
                    );
                }
                CursorShape::Underline if focused => {
                    painter.line_segment(
                        [cursor_rect.left_bottom(), cursor_rect.right_bottom()],
                        Stroke::new(2.0, color),
                    );
                }
                _ => {
                    painter.rect_stroke(
                        cursor_rect,
                        0.0,
                        Stroke::new(if focused { 2.0 } else { 1.0 }, color),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }
        if focused {
            ctx.output_mut(|output| {
                output.ime = Some(egui::output::IMEOutput { rect, cursor_rect })
            });
            if !self.composition.is_empty() {
                let galley = painter.layout_no_wrap(self.composition.clone(), font, Color32::WHITE);
                let preedit_rect = Rect::from_min_size(cursor_pos, galley.size());
                painter.rect_filled(preedit_rect, 0.0, Color32::from_rgb(47, 85, 92));
                let pos = terminal_text_pos(cursor_pos, &galley, baseline);
                painter.galley(pos, galley, Color32::WHITE);
                painter.line_segment(
                    [preedit_rect.left_bottom(), preedit_rect.right_bottom()],
                    Stroke::new(1.0, Color32::WHITE),
                );
            }
        }
        if let Some(error) = session
            .error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
        {
            self.error = Some(error);
        }
        if body_expansion {
            self.expand(!self.expanded);
        }
    }

    fn prompt_settings_content(&mut self, ui: &mut egui::Ui) {
        use crate::prompt::PathStyle;
        settings_section(
            ui,
            "한 줄 프롬프트",
            "열린 세션의 다음 프롬프트부터 적용됩니다.",
        );
        let field_width = (ui.available_width() - 140.0).clamp(80.0, 230.0);
        let prompt = &mut self.edited_settings.prompt;
        let enabled = ui
            .checkbox(&mut prompt.enabled, "MiniTerm 프롬프트 사용")
            .on_hover_text("끄면 셸 초기화 파일에서 지정한 프롬프트를 사용합니다.");
        ui.add_enabled_ui(prompt.enabled, |ui| {
            egui::Grid::new("prompt-fields").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                for (shown, label, alias) in [
                    (&mut prompt.show_user, "사용자 이름", &mut prompt.user_label),
                    (&mut prompt.show_host, "호스트 이름", &mut prompt.host_label),
                    (&mut prompt.show_path, "경로", &mut prompt.path_label),
                ] {
                    ui.checkbox(shown, label);
                    ui.add_enabled(*shown, egui::TextEdit::singleline(alias).desired_width(field_width).hint_text("비우면 실제 정보").font(egui::TextStyle::Body))
                        .on_hover_text("별칭을 지정해 표시를 바꿀 수 있습니다. 실제 사용자·호스트·작업 폴더는 바뀌지 않습니다.");
                    ui.end_row();
                }
                ui.label("경로 표시");
                ui.add_enabled_ui(prompt.show_path && prompt.path_label.is_empty(), |ui| {
                    egui::ComboBox::from_id_salt("prompt-path-style")
                        .selected_text(if prompt.path_style == PathStyle::Full { "전체 경로 · 홈은 ~" } else { "현재 폴더 이름" })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut prompt.path_style, PathStyle::Full, "전체 경로 · 홈은 ~");
                            ui.selectable_value(&mut prompt.path_style, PathStyle::Folder, "현재 폴더 이름");
                        });
                });
                ui.end_row();
                ui.label("입력 기호");
                ui.add(egui::TextEdit::singleline(&mut prompt.symbol).desired_width(90.0))
                    .on_hover_text("예: ›, ❯, $. 비우면 기호를 숨깁니다.");
                ui.end_row();
            });
            ui.add_space(12.0);
            ui.label("미리보기");
            egui::Frame::new().fill(background(self.edited_settings.theme == "light")).corner_radius(8).inner_margin(12).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{}▏", prompt.preview())).monospace());
                });
            }).response.on_hover_text("자동 정보는 예시입니다. 세션에서는 현재 사용자·호스트·폴더가 표시됩니다. 긴 프롬프트는 창 너비에 따라 줄이 접힙니다.");
        });
        self.record_widget("settings-prompt-enabled", enabled.rect);
    }

    fn settings_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match self.settings_tab {
            0 => {
                settings_section(
                    ui,
                    "새 탭의 셸",
                    "셸·시작 폴더·스크롤백은 다음에 여는 탭부터 적용됩니다.",
                );
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.wsl, false, "Git Bash");
                    ui.radio_value(&mut self.wsl, true, "WSL Bash");
                });
                if self.wsl {
                    egui::ComboBox::from_id_salt("wsl-distribution")
                        .selected_text(if self.distribution.is_empty() {
                            "배포판 선택"
                        } else {
                            &self.distribution
                        })
                        .show_ui(ui, |ui| {
                            for distro in &self.available_distributions {
                                ui.add_enabled_ui(distro.version == 2, |ui| {
                                    ui.selectable_value(
                                        &mut self.distribution,
                                        distro.name.clone(),
                                        format!("{} · WSL{}", distro.name, distro.version),
                                    );
                                });
                            }
                        });
                    if self.available_distributions.is_empty() {
                        ui.label("등록된 배포판이 없습니다. WSL2와 Linux 배포판을 설치하세요.");
                    }
                    ui.label("Linux 사용자 (비우면 배포판 기본값)");
                    ui.text_edit_singleline(&mut self.wsl_user);
                    ui.label("Linux 시작 폴더 (~ 또는 /home/...)");
                    ui.text_edit_singleline(&mut self.wsl_directory)
                        .on_hover_text("Linux 도구의 파일 성능을 위해 /home 아래를 권장합니다. 프로젝트를 자동으로 옮기지 않습니다.");
                } else {
                    ui.label("Bash 실행 파일 (비우면 내장 Bash 또는 설치된 Git)");
                    ui.text_edit_singleline(&mut self.bash_path);
                    ui.label("Windows 시작 폴더 (비우면 사용자 홈)");
                    ui.text_edit_singleline(&mut self.bash_directory);
                }
                ui.label("새 탭 스크롤백 (줄)");
                ui.add(egui::Slider::new(
                    &mut self.edited_settings.scrollback,
                    1000..=50000,
                ));
                let alternate_scroll = ui.checkbox(&mut self.edited_settings.alternate_scroll, "전체 화면 앱에서 휠로 방향키 이동")
                    .on_hover_text("기본값은 꺼짐입니다. 켜면 마우스를 처리하지 않는 vim·less에서 이동할 수 있지만 Codex 입력 이력이 바뀔 수 있습니다. Shift+휠은 터미널 기록을 스크롤합니다.");
                self.record_widget("settings-alternate-scroll", alternate_scroll.rect);
                settings_section(ui, "글꼴과 테마", "현재 터미널에도 바로 적용됩니다.");
                ui.horizontal(|ui| {
                    ui.label("테마");
                    for (icon, theme, hint) in [
                        (ActionIcon::Moon, "dark", "다크 테마"),
                        (ActionIcon::Sun, "light", "라이트 테마"),
                    ] {
                        let button = ui.add(
                            IconButton::new(icon, hint)
                                .selected(self.edited_settings.theme == theme),
                        );
                        self.record_widget(&format!("theme-{theme}"), button.rect);
                        if button.clicked() && self.edited_settings.theme != theme {
                            self.edited_settings.theme = theme.into();
                        }
                    }
                });
                ui.label("터미널 글꼴 크기");
                let font_size = ui.add(egui::Slider::new(
                    &mut self.edited_settings.font_size,
                    10..=24,
                ));
                self.record_widget("settings-font-size", font_size.rect);
                let choices = font_choices();
                let chosen = choices
                    .iter()
                    .find(|(_, p)| p == &self.edited_settings.font_family)
                    .map(|(name, _)| name.as_str())
                    .unwrap_or("사용자 글꼴");
                egui::ComboBox::from_id_salt("terminal-font")
                    .selected_text(chosen)
                    .show_ui(ui, |ui| {
                        for (name, path) in choices {
                            ui.selectable_value(&mut self.edited_settings.font_family, path, name);
                        }
                    });
                ui.collapsing("추가 글꼴 파일", |ui| {
                    ui.label("TTF/OTF 파일 경로");
                    ui.text_edit_singleline(&mut self.edited_settings.font_family);
                });
                ui.horizontal_wrapped(|ui| {
                    for (text, color, style) in [
                        ("입력 › 한글 Abc 123", NamedColor::Cyan, ""),
                        ("응답", NamedColor::Foreground, "terminal-bold"),
                        ("코드 fn()", NamedColor::Magenta, "terminal-italic"),
                    ] {
                        let family = if style.is_empty() {
                            FontFamily::Monospace
                        } else {
                            FontFamily::Name(style.into())
                        };
                        ui.label(
                            egui::RichText::new(text)
                                .font(FontId::new(self.edited_settings.font_size as f32, family))
                                .color(resolve_color(
                                    Color::Named(color),
                                    &Default::default(),
                                    self.edited_settings.theme == "light",
                                )),
                        );
                    }
                });
            }
            1 => {
                settings_section(
                    ui,
                    "창 위치",
                    "창의 헤더를 드래그하면 위치를 옮길 수 있습니다.",
                );
                let preferences = &mut self.edited_settings.window;
                egui::ComboBox::from_id_salt("hud-monitor")
                    .selected_text(if preferences.monitor.is_empty() {
                        "주 모니터"
                    } else {
                        &preferences.monitor
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut preferences.monitor, String::new(), "주 모니터");
                        for monitor in &self.available_monitors {
                            ui.selectable_value(
                                &mut preferences.monitor,
                                monitor.name.clone(),
                                format!(
                                    "{} · {}×{}{}",
                                    monitor.name,
                                    monitor.work.width,
                                    monitor.work.height,
                                    if monitor.primary { " · 주" } else { "" }
                                ),
                            );
                        }
                    });
                if !preferences.monitor.is_empty()
                    && !self
                        .available_monitors
                        .iter()
                        .any(|monitor| monitor.name == preferences.monitor)
                {
                    ui.small("지정한 모니터가 분리되어 주 모니터를 사용합니다.");
                }
                ui.label("헤더를 드래그하면 이동하고, 창 가장자리와 모서리를 드래그하면 크기를 조절합니다.");
                if preferences.anchor.is_some() {
                    ui.horizontal(|ui| {
                        ui.label("이동한 위치 사용 중");
                        if ui
                            .add(IconButton::new(
                                ActionIcon::Return,
                                "작업 표시줄 위 기본 위치로 복귀",
                            ))
                            .clicked()
                        {
                            preferences.anchor = None;
                        }
                    });
                }
                ui.label("오른쪽 간격 (DIP)");
                ui.add(egui::Slider::new(&mut preferences.right, 0..=400));
                ui.label("아래 간격 (DIP)");
                ui.add(egui::Slider::new(&mut preferences.bottom, 0..=400));
                ui.checkbox(&mut preferences.reduced_motion, "동작 줄이기")
                    .on_hover_text("Windows에서 애니메이션을 끈 경우에도 즉시 크기를 변경합니다.");
                ui.checkbox(
                    &mut preferences.auto_collapse,
                    "다른 앱으로 전환하면 미니 화면으로 축소",
                )
                .on_hover_text(
                    "꺼도 항상 위에 표시됩니다. 이 설정은 확장 크기를 유지할지 결정합니다.",
                );
                ui.label("본문·제목을 더블 클릭하면 확장하거나 축소합니다.")
                    .on_hover_text("한 번 클릭하면 포커스만 이동합니다. Shift + 더블 클릭은 터미널의 단어 선택에 사용합니다.");
                settings_section(ui, "미니 화면", "축소했을 때 표시할 내용을 선택하세요.");
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut preferences.mini_view,
                        crate::geometry::MiniView::Input,
                        "입력줄",
                    );
                    ui.selectable_value(
                        &mut preferences.mini_view,
                        crate::geometry::MiniView::Summary,
                        "작업 요약",
                    );
                });
                if preferences.mini_view == crate::geometry::MiniView::Summary {
                    ui.checkbox(&mut preferences.mini_show_status, "실행 상태 · 계획 진행률");
                    ui.checkbox(&mut preferences.mini_show_tasks, "계획 작업 목록");
                    ui.checkbox(&mut preferences.mini_show_battery, "AI Battery (하단)");
                }
                ui.checkbox(
                    &mut preferences.mini_show_title,
                    "프로그램 제목 (상단 표시줄)",
                );
                ui.checkbox(&mut preferences.mini_show_output, "최근 활동")
                    .on_hover_text("명령문 대신 파일 확인·코드 검색·검증 같은 활동을 표시합니다.");
                settings_section(
                    ui,
                    "투명도",
                    "창과 글자가 함께 투명해집니다. 0%는 불투명입니다.",
                );
                let transparency =
                    ui.add(egui::Slider::new(&mut preferences.transparency, 0..=80).suffix("%"));
                self.record_widget("settings-transparency", transparency.rect);
                settings_section(ui, "시작과 단축키", "단축키로 창을 표시하거나 숨깁니다.");
                ui.label("전역 단축키 (비우면 사용하지 않음)");
                let shortcut = ui.text_edit_singleline(&mut self.edited_settings.shortcut);
                self.record_widget("settings-shortcut", shortcut.rect);
                ui.checkbox(
                    &mut self.edited_settings.autostart,
                    "Windows 로그인 시 MiniTerm 시작",
                );
                ui.add_space(10.0);
                egui::CollapsingHeader::new("키보드 단축키").show(ui, |ui| {
                    egui::Grid::new("keyboard-shortcuts")
                        .spacing(Vec2::new(16.0, 10.0))
                        .show(ui, |ui| {
                            for (keys, action) in [
                                ("Ctrl+Shift+T", "새 탭"),
                                ("Ctrl+Shift+W", "현재 탭 닫기 (확인)"),
                                ("Ctrl+Tab / Ctrl+Shift+Tab", "다음 / 이전 탭"),
                                ("Ctrl+Alt+1~4", "번호로 탭 선택"),
                                ("Ctrl+,", "설정"),
                                ("Ctrl+Shift+C / Ctrl+Shift+V", "선택 복사 / 붙여넣기"),
                                ("Ctrl++ / Ctrl+- / Ctrl+0", "글꼴 확대 / 축소 / 기본"),
                                ("Ctrl+Shift+↑ / ↓", "기록 위 / 아래"),
                                ("Ctrl+Shift+PageUp / PageDown", "기록 한 페이지 이동"),
                                ("Ctrl+Shift+Home / End", "기록 처음 / 마지막"),
                                ("Ctrl+Alt+Enter", "확장 / 축소"),
                                ("Ctrl+Alt+M", "입력 / 요약 전환"),
                            ] {
                                ui.label(egui::RichText::new(keys).size(12.0));
                                ui.label(action);
                                ui.end_row();
                            }
                        });
                });
            }
            3 => self.prompt_settings_content(ui),
            4 => {
                settings_section(
                    ui,
                    "Codex의 Bash 실행",
                    "Windows Codex의 기본 실행 셸을 Git Bash로 연결합니다.",
                );
                ui.label("Bash 명령을 사용하면서 기존 대화를 같은 Codex 서버에서 이어갑니다.");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    for (label, enabled) in [("Bash 연결 적용", true), ("연결 해제", false)] {
                        let button = ui.button(label);
                        self.record_widget(&format!("settings-codex-bash-{enabled}"), button.rect);
                        if button.clicked() && self.settings_gate.claim(ctx.cumulative_frame_nr()) {
                            let selected = (!self.wsl && !self.bash_path.trim().is_empty()).then(|| std::path::Path::new(self.bash_path.trim()));
                            let result = std::env::current_exe().map_err(|e| e.to_string()).and_then(|helper| crate::codex_bash::configure_user_with_bash(&helper, selected, enabled));
                            self.integration_feedback = Some(match result {
                                Ok(()) => (true, "Codex 실행 환경을 변경했습니다. 실행 중인 Codex를 모두 닫은 뒤 공유 서버를 다시 시작하세요.".into()),
                                Err(error) => (false, error),
                            });
                        }
                    }
                });
                ui.add_space(16.0);
                ui.label("MiniTerm 밖에서 실행하는 Windows Codex에도 적용됩니다.");
                ui.label("모델·승인·접근 권한과 기존 지침은 유지합니다.");
                ui.add_space(16.0);
                ui.label("Codex를 모두 종료한 뒤 서버를 다시 시작하는 명령:");
                ui.monospace("codex app-server daemon restart");
            }
            _ => {
                settings_section(ui, "팝업 알림", "다른 앱·탭을 보고 있을 때만 알려줍니다.");
                let notifications =
                    ui.checkbox(&mut self.edited_settings.notifications, "Windows 팝업 표시");
                self.record_widget("settings-notifications", notifications.rect);
                let sound = ui.add_enabled(
                    self.edited_settings.notifications,
                    egui::Checkbox::new(
                        &mut self.edited_settings.notification_sound,
                        "팝업과 함께 소리 재생",
                    ),
                );
                self.record_widget("settings-notification-sound", sound.rect);
                ui.label("알림을 클릭하면 해당 탭으로 돌아갑니다.");
                ui.label(
                    egui::RichText::new("팝업을 꺼도 받은 알림은 트레이의 알림 목록에 남습니다.")
                        .weak(),
                );
                if !notification::is_registered() {
                    ui.label("이 실행 파일의 Windows 알림 등록이 필요합니다.");
                    if ui.button("Windows에 알림 등록").clicked()
                        && self.settings_gate.claim(ctx.cumulative_frame_nr())
                    {
                        self.integration_feedback = Some(
                            match std::env::current_exe()
                                .map_err(|e| e.to_string())
                                .and_then(|path| notification::register(&path))
                            {
                                Ok(()) => (true, "Windows 알림을 등록했습니다.".into()),
                                Err(error) => (false, error),
                            },
                        );
                    }
                }
                settings_section(
                    ui,
                    "CLI 응답 알림 등록",
                    "등록한 CLI의 응답 상태를 MiniTerm에서 알려줍니다.",
                );
                for (name, agent, description) in [
                    (
                        "Codex",
                        crate::agent::Agent::Codex,
                        "응답 완료·승인 요청을 해당 탭에 알립니다. 등록 후 Codex를 새로 실행하세요.",
                    ),
                    (
                        "Claude Code",
                        crate::agent::Agent::Claude,
                        "응답 종료·응답 오류·승인이나 입력이 필요할 때 알려줍니다.",
                    ),
                ] {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(name).strong());
                        for (label, enable) in [
                            ("알림 등록", true),
                            ("등록 해제", false),
                        ] {
                            let button = ui.button(label);
                            self.record_widget(&format!("settings-integration-{}-{enable}", if name == "Codex" { "codex" } else { "claude" }), button.rect);
                            if button.clicked()
                                && self.settings_gate.claim(ctx.cumulative_frame_nr())
                            {
                                let result = std::env::current_exe()
                                    .map_err(|e| e.to_string())
                                    .and_then(|path| {
                                        crate::integration::configure_user(agent, &path, enable)
                                    });
                                self.integration_feedback = Some(match result {
                                    Ok(()) => (true, if enable && agent == crate::agent::Agent::Codex {
                                        "Codex 알림을 등록했습니다. 다음 실행부터 해당 탭에 알려줍니다.".into()
                                    } else if enable {
                                        format!("{name} 알림을 등록했습니다. 새 MiniTerm 탭에서 {name}를 시작하세요.")
                                    } else {
                                        format!("{name} 알림 등록을 해제했습니다. 다음 실행부터 적용됩니다.")
                                    }),
                                    Err(error) => (false, error),
                                });
                            }
                        }
                    });
                    ui.label(egui::RichText::new(description).weak());
                }
                ui.add_space(10.0);
                ui.label(
                    "등록 후 새 탭에서 CLI를 시작하세요. 응답 종료는 작업 성공을 뜻하지 않습니다.",
                );
                ui.label(egui::RichText::new("Windows CLI의 알림 설정을 추가하고 기존 설정은 보존합니다. WSL 안의 CLI는 지원하지 않습니다.").weak());
            }
        }
    }

    fn settings_viewport(&mut self, parent_ctx: &Context) {
        if !self.settings_open {
            return;
        }
        if self.settings_observed != self.saved_settings {
            self.edited_settings = self
                .edited_settings
                .with_edits(&self.settings_observed, &self.saved_settings);
            self.settings_observed = self.saved_settings.clone();
        }
        let mut builder = egui::ViewportBuilder::default()
            .with_title("MiniTerm 설정")
            .with_icon(crate::brand::window_icon())
            .with_inner_size(SETTINGS_SIZE)
            .with_min_inner_size(SETTINGS_SIZE)
            .with_max_inner_size(SETTINGS_SIZE)
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top();
        if let Some(position) = self.settings_position {
            builder = builder.with_position(position);
        }
        let mut close = false;
        parent_ctx.show_viewport_immediate(settings_viewport_id(), builder, |ctx, _| {
            if !self.diagnostic_settings_keys.is_empty() {
                let keys = std::mem::take(&mut self.diagnostic_settings_keys);
                ctx.input_mut(|input| input.events.extend(keys));
            }
            if !self.settings_native_ready {
                self.settings_native_ready = native::relay_settings_paints(self.hwnd);
            }
            ctx.set_visuals(hud_visuals(self.settings.theme == "light"));
            let before = self.edited_settings.clone();
            let panel = egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(background(self.settings.theme == "light"))
                        .stroke(Stroke::new(
                            1.0,
                            ctx.style().visuals.widgets.noninteractive.bg_stroke.color,
                        ))
                        // The native window owns its outer corner. A rounded
                        // frame inside an opaque rectangular surface exposes
                        // the renderer's clear color behind those corners.
                        .corner_radius(0)
                        .inner_margin(18),
                )
                .show(ctx, |ui| {
                    settings_style(ui);
                    let header = ui.horizontal(|ui| {
                        ui.add(egui::Image::new(&self.brand).fit_to_exact_size(Vec2::splat(24.0)));
                        ui.label(egui::RichText::new("설정").size(18.0).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let button = ui
                                .add(IconButton::new(ActionIcon::Close, "설정창 닫기").size(28.0));
                            self.record_widget("settings-close", button.rect);
                            close = button.clicked();
                        });
                    });
                    let drag_rect = Rect::from_min_max(
                        header.response.rect.min,
                        header.response.rect.right_bottom() - Vec2::new(40.0, 0.0),
                    );
                    if ui
                        .interact(
                            drag_rect,
                            ui.id().with("settings-drag"),
                            Sense::click_and_drag(),
                        )
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .drag_started()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        for (tab, name) in [
                            (0, "터미널"),
                            (1, "창과 동작"),
                            (2, "알림"),
                            (3, "프롬프트"),
                            (4, "Codex"),
                        ] {
                            let button = ui.selectable_label(self.settings_tab == tab, name);
                            self.record_widget(&format!("settings-tab-{tab}"), button.rect);
                            if button.clicked() {
                                self.settings_tab = tab;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    ui.separator();
                    let scroll = egui::ScrollArea::vertical()
                        .id_salt(("settings", self.settings_tab))
                        .max_height(
                            (ui.available_height()
                                - if self.settings_error.is_some()
                                    || self.integration_feedback.is_some()
                                {
                                    78.0
                                } else {
                                    0.0
                                })
                            .max(40.0),
                        )
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            self.settings_content(ui, ctx);
                        });
                    self.record_widget("settings-scroll", scroll.inner_rect);
                    if let Some(error) = &self.settings_error {
                        ui.separator();
                        let message = ui.colored_label(
                            ui.visuals().error_fg_color,
                            format!("적용하지 못했습니다: {error}"),
                        );
                        self.record_widget("settings-error", message.rect);
                    } else if let Some((ok, message)) = &self.integration_feedback {
                        ui.separator();
                        let color = if *ok {
                            ui.visuals().selection.stroke.color
                        } else {
                            ui.visuals().error_fg_color
                        };
                        let feedback = ui.colored_label(color, message);
                        self.record_widget("settings-integration-feedback", feedback.rect);
                    }
                });
            self.record_widget("settings-window", panel.response.rect);
            self.edited_settings.profile = if self.wsl {
                ShellProfile::Wsl {
                    distribution: self.distribution.clone(),
                    user: (!self.wsl_user.trim().is_empty())
                        .then(|| self.wsl_user.trim().to_owned()),
                    directory: if self.wsl_directory.trim().is_empty() {
                        "~".into()
                    } else {
                        self.wsl_directory.trim().into()
                    },
                }
            } else {
                ShellProfile::GitBash {
                    bash_path: nonempty_path(&self.bash_path),
                    directory: nonempty_path(&self.bash_directory),
                }
            };
            if before != self.edited_settings && self.settings_gate.claim(ctx.cumulative_frame_nr())
            {
                let next = self
                    .saved_settings
                    .with_edits(&before, &self.edited_settings);
                let previous_error = self.settings_error.take();
                let result = if next == self.saved_settings {
                    Ok(())
                } else {
                    self.save_preferences(ctx, next)
                };
                self.settings_error = match result {
                    Err(error) => Some(error),
                    Ok(()) if self.edited_settings == self.saved_settings => None,
                    Ok(()) => previous_error,
                };
            }

            close |= ctx.input(|input| {
                input.viewport().close_requested() || input.key_pressed(Key::Escape)
            });
        });
        if close {
            self.close_settings();
            self.terminal_focus = true;
        }
    }

    fn dialogs(&mut self, ctx: &Context) {
        if (self.quit_confirm || self.close_confirm.is_some()) && ctx.input_mut(take_plain_escape) {
            self.quit_confirm = false;
            self.close_confirm = None;
            self.terminal_focus = true;
        }
        if self.notices_open {
            let notices = self
                .broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .notices
                .clone();
            let mut open = true;
            let mut activate = None;
            egui::Window::new("에이전트 알림")
                .open(&mut open)
                .collapsible(false)
                .default_width(460.0)
                .show(ctx, |ui| {
                    ui.label("응답 완료는 에이전트의 응답 종료를 뜻합니다.");
                    if notices.is_empty() {
                        ui.label("아직 받은 알림이 없습니다. 설정의 알림에서 CLI 응답 알림을 등록하세요.");
                    }
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .show(ui, |ui| {
                            for notice in &notices {
                                ui.horizontal(|ui| {
                                    let jump = ui.add(IconButton::new(
                                        ActionIcon::Return,
                                        "이 알림의 세션 열기",
                                    ));
                                    let title = ui
                                        .add(egui::Label::new(&notice.title).sense(Sense::click()));
                                    if jump.clicked() || title.clicked() {
                                        activate = Some(notice.session_id.clone());
                                    }
                                });
                            }
                        });
                    if let Some(warning) = self
                        .toast_worker
                        .warning
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .as_ref()
                    {
                        ui.label(warning);
                    }
                });
            if let Some(id) = activate {
                self.activate_session(&id);
            }
            if !open {
                self.notices_open = false;
                self.terminal_focus = true;
            }
        }
        if self.quit_confirm || self.close_confirm.is_some() {
            let all = self.quit_confirm;
            let width = (ctx.available_rect().width() - 32.0).clamp(200.0, 320.0);
            let dialog = egui::Window::new("종료 확인")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .min_width(width)
                .max_width(width)
                .default_width(width)
                .frame(
                    egui::Frame::new()
                        .fill(background(self.settings.theme == "light"))
                        .stroke(Stroke::new(
                            1.0,
                            ctx.style().visuals.widgets.noninteractive.bg_stroke.color,
                        ))
                        .corner_radius(10)
                        .inner_margin(12),
                )
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    settings_style(ui);
                    ui.label(if all {
                        "모든 탭과 작업을 닫을까요?"
                    } else {
                        "이 탭과 작업을 닫을까요?"
                    });
                    let button_width = (ui.available_width() - ui.spacing().item_spacing.x) * 0.5;
                    ui.horizontal(|ui| {
                        let cancel = ui.add_sized([button_width, 34.0], egui::Button::new("취소"));
                        self.record_widget("close-cancel", cancel.rect);
                        if cancel.clicked() {
                            self.quit_confirm = false;
                            self.close_confirm = None;
                        }
                        let confirm = ui.add_sized(
                            [button_width, 34.0],
                            egui::Button::new(if all { "모두 닫기" } else { "닫기" }),
                        );
                        self.record_widget("close-confirm", confirm.rect);
                        if confirm.clicked() {
                            if self.quit_confirm {
                                self.quit = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else if let Some(index) = self.close_confirm.take() {
                                if index < self.sessions.len() {
                                    self.remove_session(index);
                                }
                            }
                        }
                    });
                });
            if let Some(dialog) = dialog {
                self.record_widget("close-dialog", dialog.response.rect);
            }
        }
    }
}

impl eframe::App for MiniTerm {
    fn raw_input_hook(&mut self, _ctx: &Context, input: &mut egui::RawInput) {
        // epaint uses the device's maximum texture side as its font-atlas width.
        // A 16K-wide atlas wastes resident CPU/upload memory for this small HUD.
        // Keep glyphs on demand and respect devices with a smaller texture limit.
        if let Some(side) = &mut input.max_texture_side {
            *side = (*side).min(4096);
        }
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let frame_start = Instant::now();
        if let Some(deadline) = self.transparency_save_at {
            if Instant::now() >= deadline {
                self.save_quick_transparency();
            } else {
                ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
            }
        }
        if let Some(map) = &self.diagnostic_ui {
            map.lock().unwrap_or_else(|p| p.into_inner()).clear();
        }
        for cause in ctx.repaint_causes() {
            *self.repaint_causes.entry(cause.to_string()).or_default() += 1;
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide();
        }
        while let Ok(action) = self.actions.try_recv() {
            match action {
                Action::Show => {
                    native::show(self.hwnd, true);
                    self.terminal_focus = true;
                }
                Action::Hide => {
                    self.hide();
                }
                Action::Settings => {
                    self.open_settings();
                }
                Action::Quit => {
                    self.close_settings();
                    self.expand(true);
                    self.quit_confirm = true;
                    native::show(self.hwnd, true);
                }
                Action::SmokeExit => {
                    self.quit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Action::Activate(id) => self.activate_session(&id),
                Action::Diagnostic(command) => match command.as_str() {
                    "sample-scroll" => {
                        if let Some(session) = self.sessions.get(self.selected) {
                            // Fixed VT history for scroll/viewport testing. This does
                            // not test or compensate for ConPTY keyboard delivery.
                            let mut processor: alacritty_terminal::vte::ansi::Processor =
                                alacritty_terminal::vte::ansi::Processor::new();
                            let mut term = session.term.lock();
                            processor.advance(&mut *term, b"\x1b[2J\x1b[3J\x1b[H");
                            for line in 0..180 {
                                processor.advance(
                                    &mut *term,
                                    format!("SCROLL-FIXTURE {line:03}\r\n").as_bytes(),
                                );
                            }
                            session.revision.fetch_add(1, Ordering::Release);
                            self.rows.clear();
                        }
                    }
                    command if diagnostic_key(command).is_some() => {
                        let event = diagnostic_key(command).unwrap();
                        if self.settings_open {
                            self.diagnostic_settings_keys.push(event);
                        } else {
                            ctx.input_mut(|input| input.events.push(event));
                        }
                    }
                    "hide" => self.hide(),
                    "expand" => self.expand(true),
                    "collapse" => self.expand(false),
                    "settings" => self.open_settings(),
                    "theme-light" => self.set_theme(ctx, "light"),
                    "theme-dark" => self.set_theme(ctx, "dark"),
                    "settings-close" | "settings-cancel" => self.close_settings(),
                    "confirm-quit" => {
                        self.close_settings();
                        self.expand(true);
                        self.quit_confirm = true;
                        native::show(self.hwnd, true);
                    }
                    "sample" => {
                        if let Some(session) = self.sessions.get(self.selected) {
                            self.diagnostic_sample_requested = true;
                            let _ = session.input.send(b"printf '\\033[2J\\033[3J\\033[H\\033[36mINPUT \\033[0mAbc 123 \\033[1mRESPONSE BOLD\\033[0m\\n\\033[35mCODE \\033[3mfn main()\\033[0m\\n\\033[2msecondary / dim\\033[0m\\n'\r".to_vec());
                        }
                    }
                    "sample-ui" => {
                        if let Some(session) = self.sessions.get(self.selected) {
                            let rows = session.term.lock().screen_lines();
                            let command = format!(
                                "HISTFILE=/dev/null; HISTSIZE=0; printf '\\033[2J\\033[3J\\033[H한글 세션명 ABC 123\\n\\033[30;47;1m/resume 한글 세션명 검증\\033[0m\\n\\033[7m선택한 한글 세션명\\033[0m\\nAI Battery \\033[32m❚❚❚❚\\033[90m❚❚\\033[0m 67%%\\nSymbols ▄█ ░ ─│┌┐└┘ ⣿\\n\\033[37;40m흰색 글자 / 검정 배경\\033[0m\\n\\033[39;44m기본 글자 / 파랑 배경\\033[0m\\n\\033[37mANSI white / 기본 배경\\033[0m\\n\\0337\\033[{};1H\\033[36mSELECT at bottom\\033[0m\\0338'\r",
                                rows.saturating_sub(1).max(1)
                            );
                            let _ = session.input.send(command.into_bytes());
                        }
                    }
                    "sample-summary" => {
                        if let Some(session) = self.sessions.get(self.selected) {
                            let rows = session.term.lock().screen_lines();
                            let command = format!(
                                "HISTFILE=/dev/null; HISTSIZE=0; printf '\\033]2;한글 요약 세션\\007\\033[2J\\033[3J\\033[H• Updated Plan\\n  └ [x] 화면 구조 분석\\n    [~] 한글 작업 요약에서 현재 진행 중인 작업을 읽기 쉽게 표시하고 긴 작업명은 마우스 호버 안내 하나로 확인\\n    [ ] 표시와 성능 검증\\n\\033[{};1H• Working (3s • esc to interrupt)\\033[{};1H5h limit: 35%% left · /status\\033[{};1HCodex \\033[32;1m❚❚❚❚\\033[90m❚❚\\033[0m 67%% | Claude \\033[32m❚❚❚❚❚❚\\033[0m 90%%\\033[{};1H'\r",
                                rows.saturating_sub(5).max(6), rows.saturating_sub(1).max(7), rows, rows.saturating_sub(3).max(6)
                            );
                            let _ = session.input.send(command.into_bytes());
                            self.settings.window.mini_view = crate::geometry::MiniView::Summary;
                            self.saved_settings.window.mini_view =
                                crate::geometry::MiniView::Summary;
                        }
                    }
                    "sample-summary-command" => {
                        if let Some(session) = self.sessions.get(self.selected) {
                            let rows = session.term.lock().screen_lines();
                            let command = format!(
                                "HISTFILE=/dev/null; HISTSIZE=0; printf '\\033]2;요약 활동 표시 검증\\007\\033[2J\\033[3J\\033[H• Ran $taskSourceRoot=Join-Path (Get-Location).Path \"artifacts/chooser-source\"\\n\\033[{};1H• Working (58s • esc to interrupt)\\033[{};1HCodex \\033[32;1m❚❚❚❚\\033[90m❚❚\\033[0m 67%% | Claude \\033[32m❚❚❚❚❚❚\\033[0m 90%%\\033[{};1H'\r",
                                rows.saturating_sub(5).max(6), rows, rows.saturating_sub(3).max(6)
                            );
                            let _ = session.input.send(command.into_bytes());
                        }
                    }
                    "new-session" => self.start_session(ctx),
                    "quit" => {
                        self.quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    _ => {}
                },
            }
        }
        if self.display_watch.take_shortcut() {
            self.terminal_focus = true;
            self.close_settings();
        }
        if self.display_watch.take_expansion() && self.header_gate.claim(ctx.cumulative_frame_nr())
        {
            self.terminal_focus = true;
            self.expand(!self.expanded);
        }
        while let Ok(result) = self.started_rx.try_recv() {
            self.starting = self.starting.saturating_sub(1);
            match result {
                Ok(session) => {
                    if let Err(error) = session.update_prompt(&self.saved_settings.prompt) {
                        self.error = Some(error);
                    }
                    let mut broker = self.broker.lock().unwrap_or_else(|p| p.into_inner());
                    broker.register(session.id.clone());
                    if let Some(status) = broker.sessions.get_mut(&session.id) {
                        status.exited = Some(session.exited.clone());
                    }
                    drop(broker);
                    self.sessions.push(session);
                    self.selected = self.sessions.len() - 1;
                    self.rows.clear();
                }
                Err(error) => self.error = Some(error),
            }
        }
        for index in (0..self.sessions.len()).rev() {
            if self.sessions[index].exited.load(Ordering::Acquire) {
                let error = self.sessions[index]
                    .error
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                if error.is_some() {
                    self.error = error;
                }
                self.remove_session(index);
            }
        }
        if native::focused(self.hwnd)
            && !self.composing
            && !self.settings_open
            && !self.notices_open
            && !self.quit_confirm
            && self.close_confirm.is_none()
            && self.error.is_none()
        {
            let shortcuts = ctx.input_mut(take_hud_shortcuts);
            if !shortcuts.is_empty() && self.shortcut_gate.claim(ctx.cumulative_frame_nr()) {
                for shortcut in shortcuts {
                    self.hud_shortcut(ctx, shortcut);
                    if self.settings_open || self.close_confirm.is_some() {
                        break;
                    }
                }
            }
        }
        if !self.settings_open
            && !self.quit_confirm
            && self.close_confirm.is_none()
            && self.error.is_none()
        {
            let next =
                ctx.input_mut(|input| take_opacity_wheel(input, self.settings.window.transparency));
            if let Some(next) = next {
                if self.scroll_gate.claim(ctx.cumulative_frame_nr()) {
                    self.quick_transparency(next, ctx);
                }
            }
        }
        if let Some(id) = self.pending_activation.clone() {
            if self.sessions.iter().any(|session| session.id == id) {
                self.activate_session(&id);
            } else if !self
                .broker
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .sessions
                .contains_key(&id)
            {
                self.pending_activation = None;
                self.error = Some("알림의 세션을 시작하지 못했습니다.".into());
            }
        }
        if self.display_watch.take_moved() {
            self.transition = None;
            let placement = native::dragged_position(self.hwnd).and_then(|(monitor, anchor)| {
                native::window_bounds(self.hwnd).map(|bounds| (monitor, anchor, bounds))
            });
            match placement {
                Ok((monitor, anchor, bounds)) => {
                    let scale = native::window_scale(self.hwnd);
                    let preferences = &mut self.saved_settings.window;
                    preferences.monitor = monitor.clone();
                    preferences.anchor = Some(anchor.clone());
                    if self.expanded {
                        preferences.mini_width =
                            (bounds.width as f64 / scale).round().clamp(280.0, 1000.0) as u16;
                        preferences.expanded_height = Some(
                            (bounds.height as f64 / scale).round().clamp(96.0, 10_000.0) as u16,
                        );
                    } else {
                        preferences.mini_width =
                            (bounds.width as f64 / scale).round().clamp(280.0, 1000.0) as u16;
                        preferences.mini_height =
                            (bounds.height as f64 / scale).round().clamp(96.0, 320.0) as u16;
                    }
                    self.settings.window.monitor = monitor;
                    self.settings.window.anchor = Some(anchor);
                    self.settings.window.mini_width = preferences.mini_width;
                    self.settings.window.mini_height = preferences.mini_height;
                    self.settings.window.expanded_height = preferences.expanded_height;
                    if let Err(error) = self.saved_settings.save(&self.settings_path) {
                        self.error = Some(error);
                    }
                    self.rows.clear();
                }
                Err(error) => self.error = Some(error),
            }
        }
        let scale = ctx.pixels_per_point() as f64;
        if scale != self.last_scale || self.display_watch.take_changed() {
            self.last_scale = scale;
            self.rows.clear();
            self.transition = None;
            if !self.display_watch.moving() {
                if let Err(error) =
                    native::position(self.hwnd, scale, self.expanded, &self.settings.window)
                {
                    self.error = Some(error);
                }
            }
        }
        let focused = native::focused(self.hwnd);
        if self.diagnostic_sample_requested {
            if let Some(session) = self.sessions.get(self.selected) {
                // Only a namespaced fixture may request this fixed sample. Publish a
                // boolean proof, never terminal text or data from the user's session.
                let term = session.term.lock();
                let mut input = false;
                let mut response = false;
                let mut code = false;
                let mut dim = false;
                for row in 0..term.screen_lines() {
                    let line = &term.grid()[Line(row as i32)];
                    let text = (0..term.columns())
                        .map(|col| line[Column(col)].c)
                        .collect::<String>();
                    input |= text.contains("INPUT Abc 123")
                        && (0..term.columns()).any(|col| {
                            line[Column(col)].c == 'I'
                                && line[Column(col)].fg == Color::Named(NamedColor::Cyan)
                        });
                    response |= text.contains("RESPONSE BOLD")
                        && (0..term.columns()).any(|col| {
                            line[Column(col)].c == 'R'
                                && line[Column(col)].flags.contains(Flags::BOLD)
                        });
                    code |= text.contains("CODE fn main()")
                        && (0..term.columns()).any(|col| {
                            line[Column(col)].c == 'f'
                                && line[Column(col)].flags.contains(Flags::ITALIC)
                        });
                    dim |= text.contains("secondary / dim")
                        && (0..term.columns()).any(|col| {
                            line[Column(col)].c == 's'
                                && line[Column(col)].flags.contains(Flags::DIM)
                        });
                }
                self.diagnostic_sample_ready
                    .store(input && response && code && dim, Ordering::Release);
            }
        }
        if self.was_focused
            && !focused
            && self.expanded
            && self.settings.window.auto_collapse
            && !self.display_watch.moving()
            && !self.opacity_open
            && !self.settings_open
        {
            self.expand(false);
            self.close_settings();
            self.quit_confirm = false;
            self.close_confirm = None;
        }
        self.was_focused = focused;
        {
            let mut broker = self.broker.lock().unwrap_or_else(|p| p.into_inner());
            broker.selected = self
                .sessions
                .get(self.selected)
                .map(|session| session.id.clone());
            if focused && native::visible(self.hwnd) {
                if let Some(id) = broker.selected.clone() {
                    broker.read(&id);
                }
            }
        }
        let visible = native::visible(self.hwnd);
        for (index, session) in self.sessions.iter().enumerate() {
            session
                .light
                .store(self.settings.theme == "light", Ordering::Release);
            session
                .visible
                .store(visible && index == self.selected, Ordering::Release);
        }
        if !visible {
            self.settings_viewport(ctx);
            return;
        }
        if let Some(transition) = &self.transition {
            let progress = transition.started.elapsed().as_secs_f64() / 0.180;
            let bounds =
                crate::geometry::interpolate_anchored(transition.from, transition.to, progress);
            if let Err(error) = native::set_bounds(self.hwnd, bounds) {
                self.transition = None;
                self.error = Some(error);
            } else if progress >= 1.0 {
                self.transition = None;
                self.rows.clear();
                ctx.request_repaint();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(8));
            }
        }
        if self.error.is_some() && !self.expanded {
            self.expand(true);
        }
        ctx.set_visuals(hud_visuals(self.settings.theme == "light"));
        egui::TopBottomPanel::top("header")
            .exact_height(27.0)
            .frame(
                egui::Frame::new()
                    .fill(if self.settings.theme == "light" {
                        Color32::from_rgb(228, 233, 238)
                    } else {
                        Color32::from_rgb(24, 31, 42)
                    })
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show(ctx, |ui| self.header(ui, ctx));
        for (index, session) in self.sessions.iter().enumerate() {
            session.visible.store(
                native::visible(self.hwnd) && index == self.selected,
                Ordering::Release,
            );
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(background(self.settings.theme == "light"))
                    .inner_margin(egui::Margin::symmetric(9, 4)),
            )
            .show(ctx, |ui| self.terminal(ui, ctx));
        self.dialogs(ctx);
        self.settings_viewport(ctx);
        self.opacity_popup(ctx);
        if let Err(error) = self
            .appearance
            .apply(&self.display_watch, &self.settings.window)
        {
            self.error = Some(error);
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("MiniTerm 안내")
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label(&error);
                    if ui
                        .add(IconButton::new(ActionIcon::Check, "안내 닫기").size(26.0))
                        .clicked()
                    {
                        self.error = None;
                    }
                });
        }
        if self.diagnostic_ui.is_some() {
            let tooltips = ctx.memory(|memory| {
                memory
                    .areas()
                    .visible_layer_ids()
                    .iter()
                    .filter(|layer| layer.order == egui::Order::Tooltip)
                    .filter_map(|layer| memory.area_rect(layer.id))
                    .collect::<Vec<_>>()
            });
            for (index, rect) in tooltips.into_iter().enumerate() {
                self.record_widget(&format!("fixture-tooltip-{index}"), rect);
            }
        }
        if self.frame_samples.len() < 10000 {
            self.frame_samples
                .push(frame_start.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

fn nonempty_path(value: &str) -> Option<PathBuf> {
    (!value.trim().is_empty()).then(|| value.trim().into())
}

fn selection_after_removal(selected: usize, removed: usize, remaining: usize) -> usize {
    selected
        .saturating_sub(usize::from(removed < selected))
        .min(remaining.saturating_sub(1))
}

fn opacity_modifiers(modifiers: egui::Modifiers) -> bool {
    modifiers.ctrl && modifiers.alt && !modifiers.shift
}

fn hud_key(key: Key, modifiers: egui::Modifiers) -> Option<HudShortcut> {
    use HudShortcut::*;
    // consume_key ignores extra Shift/Alt; match exactly to preserve shell/TUI
    // chords such as Ctrl+W and Ctrl+Alt+Shift+W. Windows command duplicates Ctrl.
    if !modifiers.ctrl || modifiers.mac_cmd {
        return None;
    }
    match (modifiers.shift, modifiers.alt, key) {
        (true, false, Key::T) => Some(NewTab),
        (true, false, Key::W) => Some(CloseTab),
        (false, false, Key::Tab) => Some(NextTab),
        (true, false, Key::Tab) => Some(PreviousTab),
        (false, true, Key::Num1) => Some(SelectTab(0)),
        (false, true, Key::Num2) => Some(SelectTab(1)),
        (false, true, Key::Num3) => Some(SelectTab(2)),
        (false, true, Key::Num4) => Some(SelectTab(3)),
        (false, false, Key::Comma) => Some(Settings),
        (false, true, Key::Enter) => Some(ToggleExpanded),
        (false, true, Key::M) => Some(ToggleMiniView),
        (_, false, Key::Plus) | (false, false, Key::Equals) => Some(FontSize(1)),
        (false, false, Key::Minus) => Some(FontSize(-1)),
        (false, false, Key::Num0) => Some(FontSize(0)),
        (true, false, Key::ArrowUp) => Some(ScrollLines(3)),
        (true, false, Key::ArrowDown) => Some(ScrollLines(-3)),
        (true, false, Key::PageUp) => Some(ScrollPage(true)),
        (true, false, Key::PageDown) => Some(ScrollPage(false)),
        (true, false, Key::Home) => Some(ScrollEnd(true)),
        (true, false, Key::End) => Some(ScrollEnd(false)),
        _ => None,
    }
}

fn take_hud_shortcuts(input: &mut egui::InputState) -> Vec<HudShortcut> {
    let mut shortcuts = Vec::new();
    input.events.retain(|event| {
        if let Event::Key {
            key,
            modifiers,
            pressed,
            repeat,
            ..
        } = event
        {
            if let Some(shortcut) = hud_key(*key, *modifiers) {
                if *pressed
                    && (!repeat
                        || matches!(
                            shortcut,
                            HudShortcut::FontSize(_)
                                | HudShortcut::ScrollLines(_)
                                | HudShortcut::ScrollPage(_)
                                | HudShortcut::ScrollEnd(_)
                        ))
                {
                    shortcuts.push(shortcut);
                }
                return false;
            }
        }
        true
    });
    shortcuts
}

// A small, namespaced GUI fixture surface. It inserts the same egui key events
// consumed by the real selector, rather than directly invoking app actions.
fn diagnostic_key(command: &str) -> Option<Event> {
    let chord = command.strip_prefix("key:")?;
    let mut parts = chord.split('+').collect::<Vec<_>>();
    let key = Key::from_name(parts.pop()?)?;
    let mut modifiers = egui::Modifiers::NONE;
    for part in parts {
        let flag = match part {
            "ctrl" => &mut modifiers.ctrl,
            "shift" => &mut modifiers.shift,
            "alt" => &mut modifiers.alt,
            _ => return None,
        };
        if *flag {
            return None;
        }
        *flag = true;
    }
    modifiers.command = modifiers.ctrl;
    let allowed = hud_key(key, modifiers).is_some()
        || (key == Key::W && modifiers.ctrl && !modifiers.alt && !modifiers.shift)
        || (matches!(key, Key::Enter | Key::Escape) && modifiers == egui::Modifiers::NONE);
    allowed.then_some(Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    })
}

fn take_plain_escape(input: &mut egui::InputState) -> bool {
    let mut pressed = false;
    input.events.retain(|event| {
        if let Event::Key {
            key: Key::Escape,
            modifiers,
            pressed: down,
            ..
        } = event
        {
            if !modifiers.ctrl && !modifiers.alt && !modifiers.shift && !modifiers.mac_cmd {
                pressed |= *down;
                return false;
            }
        }
        true
    });
    pressed
}

fn take_opacity_wheel(input: &mut egui::InputState, current: u16) -> Option<u16> {
    if input.pointer.hover_pos().is_none()
        || !opacity_modifiers(input.modifiers)
        || input.raw_scroll_delta.y == 0.0
    {
        return None;
    }
    let next = (current as i32
        + if input.raw_scroll_delta.y > 0.0 {
            5
        } else {
            -5
        })
    .clamp(0, 80) as u16;
    input.raw_scroll_delta = Vec2::ZERO;
    input.smooth_scroll_delta = Vec2::ZERO;
    input
        .events
        .retain(|event| !matches!(event, Event::MouseWheel { .. }));
    Some(next)
}

fn alternate_wheel_bytes(
    mode: TermMode,
    shift: bool,
    enabled: bool,
    scroll: f32,
) -> Option<Vec<u8>> {
    if !enabled || shift || !mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
        return None;
    }
    let key = if scroll > 0.0 {
        Key::ArrowUp
    } else {
        Key::ArrowDown
    };
    Some(key_bytes(key, egui::Modifiers::NONE, mode)?.repeat(3))
}

impl Drop for MiniTerm {
    fn drop(&mut self) {
        crate::codex_link::close_all();
        self.save_quick_transparency();
        let Some(path) = std::env::var_os("MINITERM_METRICS_PATH").map(PathBuf::from) else {
            return;
        };
        let mut samples = self.frame_samples.clone();
        samples.sort_by(f64::total_cmp);
        let percentile = |fraction: f64| {
            samples
                .get(((samples.len() as f64 * fraction).ceil() as usize).saturating_sub(1))
                .copied()
        };
        let font_atlas_size = (!self.frame_samples.is_empty()).then(|| {
            self.display_watch
                .context()
                .fonts(|fonts| fonts.font_image_size())
        });
        let metrics = serde_json::json!({
            "version": env!("CARGO_PKG_VERSION"), "renderer": "wgpu / DX12", "adapter": self.adapter_name,
            "uiFrameSamples": samples.len(), "uiCpuMedianMs": percentile(0.5), "uiCpuP95Ms": percentile(0.95),
            "repaintCauses": self.repaint_causes,
            "startupFocusPreserved": self.startup_focus_preserved,
            "sessions": self.sessions.len(), "pendingStarts": self.starting,
            "fontAtlasSize": font_atlas_size,
            "note": "UI construction CPU time only; excludes GPU present and input-to-pixel latency. No terminal content is recorded."
        });
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&metrics) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

fn galley_baseline(galley: &egui::Galley) -> f32 {
    galley
        .rows
        .first()
        .and_then(|r| r.glyphs.first().map(|g| r.pos.y + g.pos.y))
        .unwrap_or(0.0)
}

fn summary_label(
    ui: &mut egui::Ui,
    rect: Rect,
    text: egui::RichText,
    hint: &str,
    tooltip_size: Vec2,
    role: &str,
) -> egui::Response {
    let mut elided = false;
    let response = ui.put(rect, |ui: &mut egui::Ui| {
        let (pos, galley, response) = egui::Label::new(text)
            .truncate()
            .show_tooltip_when_elided(false)
            .selectable(false)
            .layout_in_ui(ui);
        elided = galley.elided;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), galley.text())
        });
        ui.painter().galley(pos, galley, ui.visuals().text_color());
        response
    });
    if !elided || hint.trim().is_empty() {
        return response;
    }
    response.on_hover_ui(|ui| {
        ui.set_max_width(tooltip_size.x);
        egui::ScrollArea::vertical()
            .id_salt(role)
            .max_height(tooltip_size.y)
            .show(ui, |ui| {
                ui.label(hint);
            });
    })
}

fn terminal_text_pos(pos: Pos2, galley: &egui::Galley, baseline: f32) -> Pos2 {
    pos + Vec2::new(0.0, baseline - galley_baseline(galley))
}

fn font_choices() -> Vec<(String, String)> {
    let root =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("Fonts");
    let mut choices = vec![("Consolas · 기본".into(), String::new())];
    for (name, file) in [
        ("Cascadia Mono", "CascadiaMono.ttf"),
        ("Cascadia Code", "CascadiaCode.ttf"),
        ("Courier New", "cour.ttf"),
        ("Lucida Console", "lucon.ttf"),
        ("D2Coding", "D2Coding.ttf"),
        ("JetBrains Mono", "JetBrainsMono-Regular.ttf"),
    ] {
        let path = root.join(file);
        if path.is_file() {
            choices.push((name.into(), path.to_string_lossy().into_owned()));
        }
    }
    choices
}

static SYSTEM_FONTS: LazyLock<BTreeMap<&'static str, Arc<FontData>>> = LazyLock::new(|| {
    let root =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("Fonts");
    [
        "consola.ttf",
        "consolab.ttf",
        "consolai.ttf",
        "consolaz.ttf",
        "malgun.ttf",
        "malgunbd.ttf",
        "seguisym.ttf",
    ]
    .into_iter()
    .filter_map(|file| {
        // Fixed OS faces share read-only file-backed pages. User-selected fonts
        // below stay owned so replacing a custom font releases its storage.
        let bytes = crate::font_store::system_face(&root.join(file))?;
        Some((file, Arc::new(FontData::from_static(bytes))))
    })
    .collect()
});

fn install_fonts(ctx: &Context, custom: &str) {
    let mut fonts = FontDefinitions::default();
    let mono = if custom.trim().is_empty() {
        SYSTEM_FONTS.get("consola.ttf").cloned()
    } else {
        std::fs::read(custom)
            .ok()
            .map(|bytes| Arc::new(FontData::from_owned(bytes)))
    };
    if let Some(data) = mono {
        fonts.font_data.insert("terminal".into(), data);
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "terminal".into());
    }
    if let Some(data) = SYSTEM_FONTS.get("malgun.ttf") {
        fonts.font_data.insert("hangul".into(), data.clone());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("hangul".into());
        // One system UI face supplies both Latin and Hangul metrics. Keep the
        // selected terminal font and its Hangul fallback in the mono family.
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "hangul".into());
    }
    // Consolas and Malgun omit U+275A (AI Battery's bar) and other TUI symbols.
    // Use the OS symbol face as a fallback without changing the selected text font.
    if let Some(data) = SYSTEM_FONTS.get("seguisym.ttf") {
        fonts.font_data.insert("symbols".into(), data.clone());
        for family in [FontFamily::Monospace, FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("symbols".into());
        }
    }
    let fallback = fonts.families[&FontFamily::Monospace].clone();
    if let Some(data) = SYSTEM_FONTS.get("malgunbd.ttf") {
        fonts.font_data.insert("hangul-bold".into(), data.clone());
    }
    for (name, file) in [
        ("terminal-bold", "consolab.ttf"),
        ("terminal-italic", "consolai.ttf"),
        ("terminal-bolditalic", "consolaz.ttf"),
    ] {
        let mut faces = fallback.clone();
        if name != "terminal-italic" && fonts.font_data.contains_key("hangul-bold") {
            for face in &mut faces {
                if face == "hangul" {
                    *face = "hangul-bold".into();
                }
            }
        }
        if custom.trim().is_empty() {
            if let Some(data) = SYSTEM_FONTS.get(file) {
                fonts.font_data.insert(name.into(), data.clone());
                faces.insert(0, name.into());
            }
        }
        fonts.families.insert(FontFamily::Name(name.into()), faces);
    }
    ctx.set_fonts(fonts);
}

fn background(light: bool) -> Color32 {
    if light {
        Color32::from_rgb(245, 247, 250)
    } else {
        Color32::from_rgb(17, 21, 29)
    }
}

fn hud_visuals(light: bool) -> egui::Visuals {
    let mut visuals = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    let color = |index| {
        let color = crate::palette::rgb(index, light);
        Color32::from_rgb(color.r, color.g, color.b)
    };
    let surface = if light {
        Color32::from_rgb(228, 233, 238)
    } else {
        Color32::from_rgb(24, 31, 42)
    };
    let border = if light {
        Color32::from_rgb(198, 207, 219)
    } else {
        Color32::from_rgb(48, 62, 80)
    };
    let hover = if light {
        Color32::from_rgb(215, 228, 231)
    } else {
        Color32::from_rgb(34, 48, 59)
    };
    visuals.panel_fill = background(light);
    visuals.window_fill = background(light);
    visuals.window_stroke = Stroke::new(1.0, border);
    visuals.extreme_bg_color = if light {
        Color32::WHITE
    } else {
        Color32::from_rgb(13, 17, 25)
    };
    visuals.faint_bg_color = surface;
    visuals.override_text_color = Some(color(256));
    visuals.selection.bg_fill = color(6).gamma_multiply(0.18);
    visuals.selection.stroke = Stroke::new(1.0, color(6));
    for widgets in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
    ] {
        widgets.bg_fill = surface;
        widgets.weak_bg_fill = surface;
        widgets.bg_stroke = Stroke::new(1.0, border);
        widgets.fg_stroke = Stroke::new(1.0, color(256));
        widgets.corner_radius = egui::CornerRadius::same(6);
    }
    for widgets in [
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widgets.bg_fill = hover;
        widgets.weak_bg_fill = hover;
        widgets.bg_stroke = Stroke::new(1.0, color(6));
        widgets.fg_stroke = Stroke::new(1.0, color(256));
        widgets.corner_radius = egui::CornerRadius::same(6);
    }
    visuals
}

fn settings_style(ui: &mut egui::Ui) {
    let width = ui.available_width();
    let style = ui.style_mut();
    style.spacing.item_spacing = Vec2::new(12.0, 10.0);
    style.spacing.interact_size.y = 28.0;
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.slider_width = (width - 120.0).clamp(80.0, 180.0);
    style.spacing.text_edit_width = (width - 8.0).clamp(80.0, 300.0);
    for (text_style, size) in [
        (egui::TextStyle::Body, 14.0),
        (egui::TextStyle::Button, 14.0),
        (egui::TextStyle::Monospace, 14.0),
        (egui::TextStyle::Small, 12.0),
        (egui::TextStyle::Heading, 16.0),
    ] {
        style.text_styles.insert(
            text_style.clone(),
            FontId::new(
                size,
                if text_style == egui::TextStyle::Monospace {
                    FontFamily::Monospace
                } else {
                    FontFamily::Proportional
                },
            ),
        );
    }
}

fn settings_section(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new(title)
            .size(16.0)
            .strong()
            .color(ui.visuals().selection.stroke.color),
    );
    ui.label(egui::RichText::new(description).weak());
    ui.add_space(2.0);
}

fn window_placement_changed(
    a: &crate::geometry::WindowPreferences,
    b: &crate::geometry::WindowPreferences,
) -> bool {
    a.monitor != b.monitor
        || a.anchor != b.anchor
        || a.right != b.right
        || a.bottom != b.bottom
        || a.mini_width != b.mini_width
        || a.mini_height != b.mini_height
        || a.expanded_width_percent != b.expanded_width_percent
        || a.expanded_height_percent != b.expanded_height_percent
        || a.expanded_height != b.expanded_height
}

fn resolve_color(
    color: Color,
    overrides: &alacritty_terminal::term::color::Colors,
    light: bool,
) -> Color32 {
    let rgb = crate::palette::resolve(color, overrides, light);
    Color32::from_rgb(rgb.r, rgb.g, rgb.b)
}

fn resolve_cell_colors(
    foreground: Color,
    background: Color,
    flags: Flags,
    overrides: &alacritty_terminal::term::color::Colors,
    light: bool,
) -> (Color32, Color32) {
    let (fg, bg) = crate::palette::cell_colors(
        foreground,
        background,
        flags.contains(Flags::INVERSE),
        overrides,
        light,
    );
    (
        Color32::from_rgb(fg.r, fg.g, fg.b),
        Color32::from_rgb(bg.r, bg.g, bg.b),
    )
}

pub fn paste_bytes(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        format!("\u{1b}[200~{}\u{1b}[201~", text.replace('\u{1b}', "")).into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

pub fn key_bytes(key: Key, modifiers: egui::Modifiers, mode: TermMode) -> Option<Vec<u8>> {
    // Copy/Paste/Cut have separate native events. Ctrl+C still goes to the foreground program.
    if modifiers.ctrl && matches!(key, Key::C | Key::V | Key::X) {
        return None;
    }
    if modifiers.ctrl {
        let name = key.name();
        if name.len() == 1 {
            let byte = name.as_bytes()[0].to_ascii_uppercase();
            if byte.is_ascii_uppercase() {
                return Some(vec![byte - b'@']);
            }
        }
        if key == Key::Space {
            return Some(vec![0]);
        }
    }
    let modifier =
        1 + modifiers.shift as u8 + (modifiers.alt as u8) * 2 + (modifiers.ctrl as u8) * 4;
    let arrow = match key {
        Key::ArrowUp => Some('A'),
        Key::ArrowDown => Some('B'),
        Key::ArrowRight => Some('C'),
        Key::ArrowLeft => Some('D'),
        Key::Home => Some('H'),
        Key::End => Some('F'),
        _ => None,
    };
    if let Some(final_byte) = arrow {
        return Some(
            if modifier > 1 {
                format!("\u{1b}[1;{modifier}{final_byte}")
            } else if mode.contains(TermMode::APP_CURSOR) {
                format!("\u{1b}O{final_byte}")
            } else {
                format!("\u{1b}[{final_byte}")
            }
            .into_bytes(),
        );
    }
    let seq = match key {
        Key::Enter => "\r",
        Key::Escape => "\u{1b}",
        Key::Backspace => {
            if modifiers.ctrl {
                "\u{8}"
            } else {
                "\u{7f}"
            }
        }
        Key::Tab => {
            if modifiers.shift {
                "\u{1b}[Z"
            } else {
                "\t"
            }
        }
        Key::Insert => "\u{1b}[2~",
        Key::Delete => "\u{1b}[3~",
        Key::PageUp => "\u{1b}[5~",
        Key::PageDown => "\u{1b}[6~",
        Key::F1 => "\u{1b}OP",
        Key::F2 => "\u{1b}OQ",
        Key::F3 => "\u{1b}OR",
        Key::F4 => "\u{1b}OS",
        Key::F5 => "\u{1b}[15~",
        Key::F6 => "\u{1b}[17~",
        Key::F7 => "\u{1b}[18~",
        Key::F8 => "\u{1b}[19~",
        Key::F9 => "\u{1b}[20~",
        Key::F10 => "\u{1b}[21~",
        Key::F11 => "\u{1b}[23~",
        Key::F12 => "\u{1b}[24~",
        _ => return None,
    };
    Some(if modifiers.alt && !seq.starts_with('\u{1b}') {
        format!("\u{1b}{seq}").into_bytes()
    } else {
        seq.as_bytes().to_vec()
    })
}

fn send_mouse(
    session: &Session,
    mode: TermMode,
    button: usize,
    col: usize,
    row: usize,
    pressed: bool,
) {
    let data = if mode.contains(TermMode::SGR_MOUSE) {
        format!(
            "\u{1b}[<{button};{col};{row}{}",
            if pressed { 'M' } else { 'm' }
        )
        .into_bytes()
    } else if col < 224 && row < 224 {
        vec![
            27,
            b'[',
            b'M',
            (if pressed { button } else { 3 }) as u8 + 32,
            col as u8 + 32,
            row as u8 + 32,
        ]
    } else {
        return;
    };
    let _ = session.input.send(data);
}

fn tray_icon() -> Icon {
    Icon::from_rgba(crate::brand::rgba(32), 32, 32).expect("valid embedded icon")
}

pub fn run() -> eframe::Result {
    let previous_foreground = native::foreground();
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MiniTerm")
            .with_icon(crate::brand::window_icon())
            .with_inner_size([560.0, 112.0])
            .with_decorations(false)
            .with_resizable(true)
            .with_min_inner_size([280.0, 96.0])
            .with_taskbar(false)
            .with_always_on_top()
            .with_active(false)
            .with_visible(false),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = wgpu::Backends::DX12;
        setup.power_preference = wgpu::PowerPreference::LowPower;
        let default_descriptor = setup.device_descriptor.clone();
        setup.device_descriptor = std::sync::Arc::new(move |adapter| {
            let mut descriptor = default_descriptor(adapter);
            // DX12's throughput defaults reserve 256MiB/64MiB heaps, oversized for a small resident HUD.
            descriptor.memory_hints = wgpu::MemoryHints::MemoryUsage;
            descriptor
        });
    }
    eframe::run_native(
        "MiniTerm",
        options,
        Box::new(move |cc| {
            MiniTerm::new(cc).map(|mut app| {
                app.startup_focus_preserved = native::foreground() == previous_foreground;
                Box::new(app) as Box<dyn eframe::App>
            })
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_summary_does_not_repeat_its_text_in_a_tooltip() {
        let ctx = Context::default();
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        let rect = Rect::from_min_size(Pos2::new(12.0, 12.0), Vec2::new(480.0, 20.0));
        for frame in 0..6 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(520.0, 180.0))),
                    time: Some(frame as f64 * 0.2),
                    events: vec![Event::PointerMoved(rect.center())],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        summary_label(
                            ui,
                            rect,
                            egui::RichText::new("작업 계획 없음"),
                            "작업 계획 없음",
                            Vec2::new(280.0, 70.0),
                            "summary-unreported",
                        );
                    });
                },
            );
        }
        assert_eq!(
            ctx.memory(|memory| memory
                .areas()
                .visible_layer_ids()
                .iter()
                .filter(|layer| layer.order == egui::Order::Tooltip)
                .count()),
            0
        );
    }
    #[test]
    fn terminal_management_chords_do_not_consume_shell_keys_or_extra_modifiers() {
        use egui::Modifiers as M;
        let ctx = Context::default();
        let key_event = |key, modifiers| Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        };
        let mut events = Vec::new();
        for (key, modifiers) in [
            (Key::W, M::CTRL),
            (Key::W, M::CTRL | M::SHIFT),
            (Key::Tab, M::CTRL),
            (Key::Tab, M::CTRL | M::SHIFT),
            (Key::Num4, M::CTRL | M::ALT),
            (Key::Comma, M::CTRL),
            (Key::W, M::CTRL | M::SHIFT | M::ALT),
            (Key::T, M::ALT),
        ] {
            events.push(key_event(key, modifiers));
            events.push(Event::Key {
                key,
                physical_key: Some(key),
                pressed: false,
                repeat: false,
                modifiers,
            });
        }
        events.extend([Event::Copy, Event::Paste("한글 paste".into())]);
        let _ = ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                assert_eq!(
                    ctx.input_mut(take_hud_shortcuts),
                    vec![
                        HudShortcut::CloseTab,
                        HudShortcut::NextTab,
                        HudShortcut::PreviousTab,
                        HudShortcut::SelectTab(3),
                        HudShortcut::Settings,
                    ]
                );
                ctx.input(|input| {
                    let remaining_keys = input
                        .events
                        .iter()
                        .filter_map(|event| {
                            if let Event::Key {
                                key,
                                modifiers,
                                pressed: true,
                                ..
                            } = event
                            {
                                Some((*key, *modifiers))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        remaining_keys,
                        vec![
                            (Key::W, M::CTRL),
                            (Key::W, M::CTRL | M::SHIFT | M::ALT),
                            (Key::T, M::ALT),
                        ]
                    );
                    assert!(input
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::Copy)));
                    assert!(input
                        .events
                        .iter()
                        .any(|event| matches!(event, Event::Paste(_))));
                });
            },
        );
        assert_eq!(
            key_bytes(Key::W, M::CTRL, TermMode::empty()),
            Some(vec![23])
        );
    }

    #[test]
    fn escape_cancellation_is_consumed_without_changing_modified_shell_escape() {
        let ctx = Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    Event::Key {
                        key: Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                    Event::Key {
                        key: Key::Escape,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::ALT,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                assert!(ctx.input_mut(take_plain_escape));
                assert!(!ctx.input_mut(take_plain_escape));
                assert!(ctx
                    .input(|input| input.events.iter().any(
                        |event| matches!(event, Event::Key { modifiers, .. } if modifiers.alt)
                    )));
            },
        );
    }

    #[test]
    fn hud_shortcuts_consume_their_events_before_terminal_input() {
        let ctx = Context::default();
        let _ = ctx.run(
            egui::RawInput {
                events: vec![
                    Event::Key {
                        key: Key::T,
                        physical_key: Some(Key::T),
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::CTRL
                            | egui::Modifiers::SHIFT
                            | egui::Modifiers::COMMAND,
                    },
                    Event::Text("x".into()),
                ],
                ..Default::default()
            },
            |ctx| {
                assert_eq!(ctx.input_mut(take_hud_shortcuts), vec![HudShortcut::NewTab]);
                assert!(ctx.input_mut(take_hud_shortcuts).is_empty());
                let forwarded: Vec<_> = ctx.input(|input| {
                    input
                        .events
                        .iter()
                        .filter_map(|event| match event {
                            Event::Key {
                                key,
                                pressed: true,
                                modifiers,
                                ..
                            } => key_bytes(*key, *modifiers, TermMode::empty()),
                            Event::Text(text) => Some(text.as_bytes().to_vec()),
                            _ => None,
                        })
                        .flatten()
                        .collect()
                });
                assert_eq!(
                    forwarded, b"x",
                    "Ctrl+T must not reach Bash when opening a tab"
                );
            },
        );
        for (modifiers, initial, expected) in [
            (egui::Modifiers::CTRL | egui::Modifiers::ALT, 78, Some(80)),
            (egui::Modifiers::NONE, 78, None),
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    modifiers,
                    events: vec![
                        Event::PointerMoved(Pos2::new(40.0, 40.0)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Line,
                            delta: Vec2::new(0.0, 1.0),
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    assert_eq!(
                        ctx.input_mut(|input| take_opacity_wheel(input, initial)),
                        expected
                    );
                    assert_eq!(
                        ctx.input(|input| input
                            .events
                            .iter()
                            .any(|event| matches!(event, Event::MouseWheel { .. }))),
                        expected.is_none(),
                        "Opacity adjustment must not also scroll the terminal"
                    );
                },
            );
        }
    }
    #[test]
    fn wheel_does_not_edit_codex_history_and_opt_in_respects_dec_modes() {
        let fullscreen = TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL;
        assert_eq!(alternate_wheel_bytes(fullscreen, false, false, 40.0), None);
        assert_eq!(alternate_wheel_bytes(fullscreen, true, true, 40.0), None);
        assert_eq!(
            alternate_wheel_bytes(TermMode::ALT_SCREEN, false, true, 40.0),
            None
        );
        assert_eq!(
            alternate_wheel_bytes(TermMode::ALTERNATE_SCROLL, false, true, 40.0),
            None
        );
        assert_eq!(
            alternate_wheel_bytes(fullscreen, false, true, 40.0).unwrap(),
            b"\x1b[A\x1b[A\x1b[A"
        );
        assert_eq!(
            alternate_wheel_bytes(fullscreen | TermMode::APP_CURSOR, false, true, -40.0).unwrap(),
            b"\x1bOB\x1bOB\x1bOB"
        );
    }

    #[test]
    fn closing_a_tab_preserves_the_active_session_or_selects_a_neighbor() {
        assert_eq!(selection_after_removal(2, 0, 3), 1);
        assert_eq!(selection_after_removal(1, 1, 3), 1);
        assert_eq!(selection_after_removal(3, 3, 3), 2);
        assert_eq!(selection_after_removal(0, 0, 0), 0);
    }
    #[test]
    fn elided_summary_has_one_tooltip_with_the_complete_work_label() {
        let ctx = Context::default();
        ctx.style_mut(|style| style.interaction.tooltip_delay = 0.0);
        let text = "Current task: improve readable summaries and verify hover behavior";
        let rect = Rect::from_min_size(Pos2::new(12.0, 12.0), Vec2::new(110.0, 20.0));
        for frame in 0..6 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 180.0))),
                    time: Some(frame as f64 * 0.2),
                    events: vec![Event::PointerMoved(rect.center())],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        summary_label(
                            ui,
                            rect,
                            egui::RichText::new(text),
                            text,
                            Vec2::new(280.0, 70.0),
                            "summary-task",
                        );
                    });
                },
            );
        }
        let tooltips = ctx.memory(|memory| {
            memory
                .areas()
                .visible_layer_ids()
                .iter()
                .filter(|layer| layer.order == egui::Order::Tooltip)
                .count()
        });
        assert_eq!(tooltips, 1, "Elided labels must not add a second tooltip");
    }
    #[test]
    fn passes_escape_and_job_control_and_app_cursor() {
        assert_eq!(
            key_bytes(Key::Escape, egui::Modifiers::NONE, TermMode::empty()).unwrap(),
            vec![27]
        );
        assert_eq!(
            key_bytes(Key::Z, egui::Modifiers::CTRL, TermMode::empty()).unwrap(),
            vec![26]
        );
        assert_eq!(
            key_bytes(Key::ArrowUp, egui::Modifiers::NONE, TermMode::APP_CURSOR).unwrap(),
            b"\x1bOA"
        );
    }
    #[test]
    fn bracketed_paste_cannot_inject_an_end_marker() {
        assert_eq!(
            paste_bytes("a\x1b[201~b", true),
            b"\x1b[200~a[201~b\x1b[201~"
        );
    }
    #[test]
    fn latin_digits_and_hangul_share_a_terminal_baseline_at_each_dpi() {
        let ctx = Context::default();
        install_fonts(&ctx, "");
        for scale in [1.0, 1.25, 2.0] {
            ctx.set_pixels_per_point(scale);
            let _ = ctx.run(Default::default(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                for size in [10.0, 13.0, 24.0] {
                    let font = FontId::monospace(size);
                    let reference =
                        painter.layout_no_wrap("M".into(), font.clone(), Color32::WHITE);
                    let height = (size * 1.38).ceil();
                    let baseline =
                        (height - reference.size().y).max(0.0) * 0.5 + galley_baseline(&reference);
                    for text in ["Abc", "123", "한글", "가나다"] {
                        let galley =
                            painter.layout_no_wrap(text.into(), font.clone(), Color32::WHITE);
                        let pos = terminal_text_pos(Pos2::ZERO, &galley, baseline);
                        assert!(
                            (pos.y + galley_baseline(&galley) - baseline).abs() < 0.01,
                            "{text} at {size}/{scale}"
                        );
                        assert!(!galley.rows[0].visuals.mesh.vertices.is_empty());
                    }
                }
            });
        }
    }
    #[test]
    fn a_font_atlas_rebuild_invalidates_the_reference_even_when_cell_metrics_match() {
        let ctx = Context::default();
        install_fonts(&ctx, "");
        let mut references = Vec::new();
        for coverage in [
            egui::epaint::AlphaFromCoverage::Linear,
            egui::epaint::AlphaFromCoverage::Linear,
            egui::epaint::AlphaFromCoverage::Gamma(0.55),
        ] {
            ctx.style_mut(|s| s.visuals.text_alpha_from_coverage = coverage);
            let _ = ctx.run(Default::default(), |ctx| {
                references.push(
                    ctx.layer_painter(egui::LayerId::background())
                        .layout_no_wrap("M".into(), FontId::monospace(13.0), Color32::WHITE),
                );
            });
        }
        assert!(Arc::ptr_eq(&references[0], &references[1]));
        assert!(!Arc::ptr_eq(&references[1], &references[2]));
        assert_eq!(references[1].size(), references[2].size());
    }
    #[test]
    fn terminal_faces_cover_battery_bars_hangul_and_braille() {
        let ctx = Context::default();
        install_fonts(&ctx, "");
        let _ = ctx.run(Default::default(), |ctx| {
            for family in [
                FontFamily::Monospace,
                FontFamily::Name("terminal-bold".into()),
                FontFamily::Name("terminal-italic".into()),
                FontFamily::Name("terminal-bolditalic".into()),
            ] {
                for glyph in ['한', '글', '❚', '⣿', '█', '─'] {
                    assert!(
                        ctx.fonts_mut(
                            |fonts| fonts.has_glyph(&FontId::new(13.0, family.clone()), glyph)
                        ),
                        "Missing {glyph} in {family:?}"
                    );
                }
            }
        });
    }
}
