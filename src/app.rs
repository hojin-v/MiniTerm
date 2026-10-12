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
    settings_gate: FrameGate,
    broker: Arc<Mutex<Broker>>,
    _ipc_server: ipc::Server,
    toast_worker: Arc<notification::ToastWorker>,
    notification_preferences: Arc<Mutex<(bool, bool)>>,
    notices_open: bool,
    diagnostic_ui: Option<DiagnosticUi>,
    diagnostic_sample_requested: bool,
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
        let diagnostic_sample_ready = Arc::new(AtomicBool::new(false));
        let ipc_sample_ready = diagnostic_sample_ready.clone();
        let ipc_server = ipc::serve(move |request| match request {
            ipc::Request::Diagnostic { command } => {
                if !diagnostics {
                    return ipc::Reply::error("진단 모드가 활성화되지 않았습니다.");
                }
                if ![
                    "hide",
                    "expand",
                    "collapse",
                    "settings",
                    "new-session",
                    "quit",
                    "theme-light",
                    "theme-dark",
                    "settings-cancel",
                    "sample",
                    "sample-ui",
                    "sample-summary",
                    "sample-summary-command",
                ]
                .contains(&command.as_str())
                {
                    return ipc::Reply::error("지원하지 않는 진단 명령입니다.");
                }
                if matches!(
                    command.as_str(),
                    "sample" | "sample-ui" | "sample-summary" | "sample-summary-command"
                ) && std::env::var_os("MINITERM_INSTANCE_ID").is_none()
                {
                    return ipc::Reply::error(
                        "출력 샘플은 별도 진단 세션에서만 사용할 수 있습니다.",
                    );
                }
                // Hidden viewports need a native visibility event to dispatch a UI action.
                if command != "hide" {
                    native::show(hwnd, false);
                }
                let _ = ipc_tx.send(Action::Diagnostic(command));
                ipc_ctx.request_repaint();
                ipc::Reply::ok()
            }
            ipc::Request::Event { event } => {
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
                let (enabled, sound) = *ipc_preferences.lock().unwrap_or_else(|p| p.into_inner());
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
                    }),
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
            if !matches!(action, Action::Hide) {
                native::show(hwnd, true);
            }
            let _ = menu_tx.send(action);
            menu_ctx.request_repaint();
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
            settings_gate: FrameGate::default(),
            broker,
            _ipc_server: ipc_server,
            toast_worker,
            notification_preferences,
            notices_open: false,
            diagnostic_ui,
            diagnostic_sample_requested: false,
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
            self.settings = self.saved_settings.clone();
            install_fonts(self.display_watch.context(), &self.settings.font_family);
            self.load_fields();
            self.rows.clear();
        }
        self.settings_open = false;
    }

    fn open_settings(&mut self) {
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
        self.expand(true);
        self.settings_open = true;
        self.terminal_focus = false;
    }

    fn save_preferences(&mut self, ctx: &Context, settings: Settings) -> Result<(), String> {
        settings.validate()?;
        settings.profile.command("validation")?;
        if !settings.font_family.trim().is_empty() {
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
        self.display_watch.register_shortcut(&settings.shortcut)?;
        let start_changed = settings.autostart != self.saved_settings.autostart;
        if start_changed {
            if let Err(error) = native::autostart(&exe, settings.autostart) {
                let _ = self
                    .display_watch
                    .register_shortcut(&self.saved_settings.shortcut);
                return Err(error);
            }
        }
        let persisted = (|| {
            for session in &self.sessions {
                session.update_prompt(&settings.prompt)?;
            }
            settings.save(&self.settings_path)
        })();
        if let Err(error) = persisted {
            for session in &self.sessions {
                let _ = session.update_prompt(&self.saved_settings.prompt);
            }
            let _ = self
                .display_watch
                .register_shortcut(&self.saved_settings.shortcut);
            if start_changed {
                let _ = native::autostart(&exe, self.saved_settings.autostart);
            }
            return Err(error);
        }
        install_fonts(ctx, &settings.font_family);
        *self
            .notification_preferences
            .lock()
            .unwrap_or_else(|p| p.into_inner()) =
            (settings.notifications, settings.notification_sound);
        self.saved_settings = settings.clone();
        self.settings = settings;
        self.transition = None;
        if let Err(error) = native::position(
            self.hwnd,
            self.last_scale,
            self.expanded,
            &self.settings.window,
        ) {
            self.error = Some(error);
        }
        self.rows.clear();
        self.settings_open = false;
        self.terminal_focus = true;
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
        self.settings.theme = theme.into();
        self.rows.clear();
        ctx.set_visuals(if theme == "light" {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        });
        ctx.request_discard("theme changed");
        ctx.request_repaint();
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
                    egui::Label::new(
                        egui::RichText::new(if compact { "›_" } else { "›_ MiniTerm" })
                            .strong()
                            .color(resolve_color(
                                Color::Named(NamedColor::Cyan),
                                &Default::default(),
                                self.settings.theme == "light",
                            )),
                    )
                    .selectable(false)
                    .sense(Sense::click_and_drag()),
                )
                .on_hover_text("드래그: 창 이동 · 더블 클릭: 확장/축소");
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
                    .on_hover_text(format!("{}\n오른쪽 클릭: 세션 종료", session.label));
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
                .show_tooltip_when_elided(false)
                .selectable(false)
                .sense(Sense::click_and_drag()),
            );
            let state = state.on_hover_text(&status);
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
                    IconButton::new(ActionIcon::Hide, "HUD 숨기기 · 셸은 계속 실행됩니다")
                        .size(action_size),
                );
                self.record_widget("header-hide", hide.rect);
                if hide.clicked() && self.header_gate.claim(frame) {
                    self.hide();
                }
                let layout = ui.add(
                    IconButton::new(
                        ActionIcon::Layout,
                        if self.settings.window.mini_view == crate::geometry::MiniView::Summary {
                            "미니 보기: 작업 요약 · 클릭하여 입력줄로 전환"
                        } else {
                            "미니 보기: 입력줄 · 클릭하여 작업 요약으로 전환"
                        },
                    )
                    .selected(self.settings.window.mini_view == crate::geometry::MiniView::Summary)
                    .size(action_size),
                );
                self.record_widget("header-layout", layout.rect);
                if layout.clicked() && self.header_gate.claim(frame) {
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
                let add = ui.add_enabled(
                    self.sessions.len() + self.starting < 4,
                    IconButton::new(ActionIcon::Add, "새 세션 · 최대 4개").size(action_size),
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
                }
            }
        }
        if battery {
            self.record_widget("summary-battery", battery_rect);
            ui.interact(
                battery_rect,
                ui.id().with("summary-battery"),
                Sense::hover(),
            )
            .on_hover_text(snapshot.battery_text.as_deref().unwrap_or_default());
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
                    "Git Bash를 ConPTY에 연결하고 있습니다…"
                } else {
                    "설정에서 Git for Windows의 bash.exe를 지정하고 + 버튼으로 세션을 여세요."
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
            .map(|snapshot| self.paint_summary(ui, snapshot, rect, &font, height))
            .unwrap_or(rect);
        let mut term = session.term.lock();
        let cols = cols.min(term.columns() as u16);
        let offset = term.grid().display_offset();
        let visible_rows: Vec<i32> = if let Some(snapshot) = &summary_snapshot {
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
        let mut column_start = if mini && !summary {
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
            if scroll.abs() > 0.0 && self.scroll_gate.claim(frame) {
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
                } else if term.mode().contains(TermMode::ALT_SCREEN) && !shift {
                    let seq = if scroll > 0.0 { "\u{1b}[A" } else { "\u{1b}[B" };
                    let _ = session.input.send(seq.repeat(3).into_bytes());
                } else {
                    term.scroll_display(Scroll::Delta((scroll / height).round() as i32));
                }
            }
        }
        if focused && !input_events.is_empty() && self.input_gate.claim(frame) {
            let mut input_batch = Vec::new();
            for event in input_events {
                let data = match event {
                    Event::Ime(ImeEvent::Preedit(text)) => {
                        self.composing = !text.is_empty();
                        self.composition = text;
                        None
                    }
                    Event::Ime(ImeEvent::Commit(text)) => {
                        self.composing = false;
                        self.composition.clear();
                        Some(text.into_bytes())
                    }
                    Event::Ime(ImeEvent::Disabled) => {
                        self.composing = false;
                        self.composition.clear();
                        None
                    }
                    Event::Text(text) if !self.composing => {
                        let alt = ctx.input(|i| i.modifiers.alt);
                        Some(if alt {
                            format!("\u{1b}{text}").into_bytes()
                        } else {
                            text.into_bytes()
                        })
                    }
                    Event::Paste(text) => Some(paste_bytes(
                        &text,
                        term.mode().contains(TermMode::BRACKETED_PASTE),
                    )),
                    Event::Copy if shift => {
                        if let Some(text) = term.selection_to_string() {
                            ctx.copy_text(text);
                        }
                        None
                    }
                    Event::Copy => Some(vec![3]),
                    Event::Cut => Some(vec![24]),
                    Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } if !self.composing => key_bytes(key, modifiers, *term.mode()),
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
                    input_batch.extend(data);
                }
            }
            if !input_batch.is_empty() {
                term.scroll_display(Scroll::Bottom);
                let submitted = input_batch.contains(&b'\r') || input_batch.contains(&3);
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
        ui.heading("한 줄 프롬프트").on_hover_text("저장하면 열린 세션의 다음 프롬프트부터 적용됩니다. 실행 중인 명령과 입력은 유지합니다.");
        let prompt = &mut self.settings.prompt;
        ui.checkbox(&mut prompt.enabled, "MiniTerm 프롬프트 사용")
            .on_hover_text("끄면 셸 초기화 파일에서 지정한 프롬프트를 사용합니다.");
        ui.add_enabled_ui(prompt.enabled, |ui| {
            egui::Grid::new("prompt-fields").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                for (shown, label, alias) in [
                    (&mut prompt.show_user, "사용자 이름", &mut prompt.user_label),
                    (&mut prompt.show_host, "호스트 이름", &mut prompt.host_label),
                    (&mut prompt.show_path, "경로", &mut prompt.path_label),
                ] {
                    ui.checkbox(shown, label);
                    ui.add_enabled(*shown, egui::TextEdit::singleline(alias).desired_width(230.0).hint_text("비우면 실제 정보"))
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
            egui::Frame::new().fill(background(self.settings.theme == "light")).corner_radius(8).inner_margin(12).show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(format!("{}▏", prompt.preview())).monospace());
                });
            }).response.on_hover_text("자동 정보는 예시입니다. 세션에서는 현재 사용자·호스트·폴더가 표시됩니다. 긴 프롬프트는 창 너비에 따라 줄이 접힙니다.");
        });
    }

    fn settings_content(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match self.settings_tab {
            0 => {
                ui.heading("새 세션의 셸")
                    .on_hover_text("셸·시작 폴더·스크롤백 변경은 새 세션에 적용됩니다.");
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
                    ui.label("Bash 실행 파일 (비우면 자동 탐색)");
                    ui.text_edit_singleline(&mut self.bash_path);
                    ui.label("Windows 시작 폴더 (비우면 사용자 홈)");
                    ui.text_edit_singleline(&mut self.bash_directory);
                }
                ui.add(
                    egui::Slider::new(&mut self.settings.scrollback, 1000..=50000)
                        .text("새 세션 스크롤백"),
                );
                ui.separator();
                ui.heading("화면");
                ui.horizontal(|ui| {
                    ui.label("테마");
                    for (icon, theme, hint) in [
                        (ActionIcon::Moon, "dark", "다크 테마"),
                        (ActionIcon::Sun, "light", "라이트 테마"),
                    ] {
                        let button = ui.add(
                            IconButton::new(icon, hint).selected(self.settings.theme == theme),
                        );
                        self.record_widget(&format!("theme-{theme}"), button.rect);
                        if button.clicked() && self.settings.theme != theme {
                            self.set_theme(ui.ctx(), theme);
                        }
                    }
                });
                ui.add(egui::Slider::new(&mut self.settings.font_size, 10..=24).text("글꼴 크기"));
                let previous_font = self.settings.font_family.clone();
                let choices = font_choices();
                let chosen = choices
                    .iter()
                    .find(|(_, p)| p == &self.settings.font_family)
                    .map(|(name, _)| name.as_str())
                    .unwrap_or("사용자 글꼴");
                egui::ComboBox::from_id_salt("terminal-font")
                    .selected_text(chosen)
                    .show_ui(ui, |ui| {
                        for (name, path) in choices {
                            ui.selectable_value(&mut self.settings.font_family, path, name);
                        }
                    });
                if previous_font != self.settings.font_family {
                    install_fonts(ui.ctx(), &self.settings.font_family);
                    self.rows.clear();
                    ui.ctx().request_repaint();
                }
                ui.collapsing("추가 글꼴 파일", |ui| {
                    ui.label("TTF/OTF 파일 경로");
                    ui.text_edit_singleline(&mut self.settings.font_family);
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
                                .font(FontId::new(self.settings.font_size as f32, family))
                                .color(resolve_color(
                                    Color::Named(color),
                                    &Default::default(),
                                    self.settings.theme == "light",
                                )),
                        );
                    }
                });
            }
            1 => {
                ui.heading("창 배치");
                let preferences = &mut self.settings.window;
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
                ui.add(
                    egui::Slider::new(&mut preferences.right, 0..=400).text("오른쪽 간격 (DIP)"),
                );
                ui.add(egui::Slider::new(&mut preferences.bottom, 0..=400).text("아래 간격 (DIP)"));
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
                ui.separator();
                ui.heading("미니 화면 표시");
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
                ui.separator();
                ui.heading("창 모양");
                ui.add(egui::Slider::new(&mut preferences.transparency, 0..=80).text("투명도 (%)"))
                    .on_hover_text("0은 불투명입니다. 창과 글자가 함께 투명해집니다. 저장 전 미리 볼 수 있고 취소하면 복원됩니다.");
                ui.separator();
                ui.label("전역 단축키 (비우면 사용하지 않음)");
                ui.text_edit_singleline(&mut self.settings.shortcut);
                ui.checkbox(
                    &mut self.settings.autostart,
                    "Windows 로그인 시 MiniTerm 시작",
                );
            }
            3 => self.prompt_settings_content(ui),
            _ => {
                ui.heading("Windows 알림");
                ui.checkbox(
                    &mut self.settings.notifications,
                    "백그라운드 세션의 Windows 알림",
                );
                ui.checkbox(&mut self.settings.notification_sound, "알림 소리");
                ui.heading("응답 알림").on_hover_text("응답 완료는 에이전트의 응답 종료를 뜻합니다. 작업의 성공 여부는 터미널에서 확인하세요.");
                if !notification::is_registered() {
                    ui.label("앱을 설치하거나 알림을 등록하세요.");
                    if ui
                        .add(IconButton::new(ActionIcon::Bell, "Windows 알림 등록").size(26.0))
                        .clicked()
                        && self.settings_gate.claim(ctx.cumulative_frame_nr())
                    {
                        if let Err(error) = std::env::current_exe()
                            .map_err(|e| e.to_string())
                            .and_then(|path| notification::register(&path))
                        {
                            self.error = Some(error);
                        }
                    }
                }
                ui.separator();
                ui.heading("Windows 에이전트 연결")
                    .on_hover_text("기존 notify·훅을 보존하며 사용자 설정에 연결을 추가합니다.");
                for (name, agent) in [
                    ("Codex", crate::agent::Agent::Codex),
                    ("Claude Code", crate::agent::Agent::Claude),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        for (icon, hint, enable) in [
                            (ActionIcon::Link, "에이전트 알림 연결", true),
                            (ActionIcon::Unlink, "에이전트 알림 연결 해제", false),
                        ] {
                            if ui.add(IconButton::new(icon, hint).size(26.0)).clicked()
                                && self.settings_gate.claim(ctx.cumulative_frame_nr())
                            {
                                let result = std::env::current_exe()
                                    .map_err(|e| e.to_string())
                                    .and_then(|path| {
                                        crate::integration::configure_user(agent, &path, enable)
                                    });
                                if let Err(error) = result {
                                    self.error = Some(error);
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    fn dialogs(&mut self, ctx: &Context) {
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
                        ui.label("아직 받은 알림이 없습니다. 설정에서 에이전트를 연결하세요.");
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
        if self.settings_open {
            let mut open = true;
            let available = ctx.available_rect().size() - Vec2::splat(24.0);
            egui::Window::new("MiniTerm 설정")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(
                    available.x.clamp(220.0, 490.0),
                    available.y.clamp(120.0, 520.0),
                ))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (tab, icon, hint) in [
                            (0, ActionIcon::Terminal, "터미널 설정"),
                            (1, ActionIcon::Layout, "창·동작 설정"),
                            (2, ActionIcon::Bell, "알림·연결 설정"),
                            (3, ActionIcon::Prompt, "프롬프트 설정"),
                        ] {
                            let button = ui.add(
                                IconButton::new(icon, hint)
                                    .selected(self.settings_tab == tab)
                                    .size(28.0),
                            );
                            self.record_widget(&format!("settings-tab-{tab}"), button.rect);
                            if button.clicked() {
                                self.settings_tab = tab;
                            }
                        }
                    });
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt(("settings", self.settings_tab))
                        .max_height((ui.available_height() - 40.0).max(60.0))
                        .auto_shrink([false, false])
                        .show(ui, |ui| self.settings_content(ui, ctx));
                    ui.separator();
                    ui.horizontal(|ui| {
                        let save =
                            ui.add(IconButton::new(ActionIcon::Save, "설정 저장").size(26.0));
                        self.record_widget("settings-save", save.rect);
                        if save.clicked() && self.settings_gate.claim(ctx.cumulative_frame_nr()) {
                            let mut settings = self.settings.clone();
                            settings.profile = if self.wsl {
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
                            if let Err(error) = self.save_preferences(ctx, settings) {
                                self.error = Some(error);
                            }
                        }
                        let cancel =
                            ui.add(IconButton::new(ActionIcon::Close, "변경 취소").size(26.0));
                        self.record_widget("settings-cancel", cancel.rect);
                        if cancel.clicked() && self.settings_gate.claim(ctx.cumulative_frame_nr()) {
                            self.close_settings();
                            self.terminal_focus = true;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    IconButton::new(ActionIcon::Power, "앱 종료…")
                                        .size(26.0)
                                        .danger(),
                                )
                                .clicked()
                            {
                                self.quit_confirm = true;
                            }
                        });
                    });
                });
            if !open {
                self.close_settings();
                self.terminal_focus = true;
            }
        }
        if self.quit_confirm || self.close_confirm.is_some() {
            egui::Window::new("종료 확인")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(if self.quit_confirm {
                        "MiniTerm을 종료하면 모든 Bash와 실행 중인 프로그램도 종료됩니다."
                    } else {
                        "이 Bash 세션과 실행 중인 프로그램을 종료할까요?"
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add(IconButton::new(ActionIcon::Close, "취소").size(26.0))
                            .clicked()
                        {
                            self.quit_confirm = false;
                            self.close_confirm = None;
                        }
                        if ui
                            .add(
                                IconButton::new(ActionIcon::Power, "종료 확인")
                                    .size(26.0)
                                    .danger(),
                            )
                            .clicked()
                        {
                            if self.quit_confirm {
                                self.quit = true;
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            } else if let Some(index) = self.close_confirm.take() {
                                if index < self.sessions.len() {
                                    self.broker
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .remove(&self.sessions[index].id);
                                    self.sessions.remove(index);
                                    self.selected =
                                        self.selected.min(self.sessions.len().saturating_sub(1));
                                    self.rows.clear();
                                }
                            }
                        }
                    });
                });
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
                    native::show(self.hwnd, true);
                }
                Action::Quit => {
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
                    "hide" => self.hide(),
                    "expand" => self.expand(true),
                    "collapse" => self.expand(false),
                    "settings" => self.open_settings(),
                    "theme-light" => self.set_theme(ctx, "light"),
                    "theme-dark" => self.set_theme(ctx, "dark"),
                    "settings-cancel" => self.close_settings(),
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
        ctx.set_visuals(if self.settings.theme == "light" {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        });
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

impl Drop for MiniTerm {
    fn drop(&mut self) {
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
    ui.put(
        rect,
        egui::Label::new(text)
            .truncate()
            .show_tooltip_when_elided(false)
            .selectable(false),
    )
    .on_hover_ui(|ui| {
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
        let bytes = std::fs::read(root.join(file)).ok()?;
        // Exactly seven fixed OS faces are process-lifetime assets. Borrowed
        // bytes let egui use FontRef instead of cloning each large TTF to FontVec.
        // Custom font paths stay owned and are released when the UI replaces them.
        let bytes = Box::leak(bytes.into_boxed_slice());
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
        for family in [FontFamily::Monospace, FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("hangul".into());
        }
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
    let mut rgba = vec![0u8; 32 * 32 * 4];
    for y in 0..32 {
        for x in 0..32 {
            let on = ((7..=14).contains(&x) && ((y as i32 - 16).abs() - (x as i32 - 7)).abs() <= 1)
                || ((18..=26).contains(&x) && (22..=24).contains(&y));
            let pixel = (y * 32 + x) * 4;
            rgba[pixel..pixel + 4].copy_from_slice(if on {
                &[106, 218, 187, 255]
            } else {
                &[17, 21, 29, 255]
            });
        }
    }
    Icon::from_rgba(rgba, 32, 32).expect("valid embedded icon")
}

pub fn run() -> eframe::Result {
    let previous_foreground = native::foreground();
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MiniTerm")
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
