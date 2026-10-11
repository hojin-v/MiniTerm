#![cfg(windows)]

#[path = "support/responses_fixture.rs"]
mod responses_fixture;

use miniterm::{native::ProcessJob, pty, shell::ShellProfile};
use portable_pty::{Child, MasterPty};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc::{sync_channel, Receiver},
    thread,
    time::{Duration, Instant},
};

struct Harness {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    job: ProcessJob,
    output: Receiver<Vec<u8>>,
    pending: Vec<u8>,
}

fn git_test_profile(directory: Option<PathBuf>) -> ShellProfile {
    ShellProfile::GitBash {
        bash_path: std::env::var_os("MINITERM_GIT_TEST_BASH").map(PathBuf::from),
        directory,
    }
}

impl Harness {
    fn new(directory: PathBuf) -> Self {
        Self::profile(git_test_profile(Some(directory)), "conpty-test")
    }

    fn profile(profile: ShellProfile, id: &str) -> Self {
        let mut harness = Self::spawn(profile, id, None);
        harness.send("HISTFILE=/dev/null; HISTSIZE=0; PS1='__MINI''TERM_PROMPT__ '; printf '__MINI''TERM_READY__\\n'\r");
        harness.until("__MINITERM_READY__");
        harness.until("__MINITERM_PROMPT__ ");
        harness
    }

    fn spawn(profile: ShellProfile, id: &str, prompt: Option<&std::path::Path>) -> Self {
        let pty::OpenPty {
            master,
            writer,
            mut reader,
            child,
            job,
        } = pty::open_with_prompt(&profile, id, 80, 24, prompt).expect("Bash / ConPTY start");
        let (tx, output) = sync_channel(16);
        thread::spawn(move || {
            let mut buffer = [0; 16384];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 || tx.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });
        Self {
            master,
            writer,
            child,
            job,
            output,
            pending: Vec::new(),
        }
    }
    fn send(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).unwrap();
        self.writer.flush().unwrap();
    }
    fn until(&mut self, marker: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(index) = self
                .pending
                .windows(4)
                .position(|bytes| bytes == b"\x1b[6n")
            {
                self.pending.drain(index..index + 4);
                self.send("\x1b[1;1R");
            }
            if let Some(index) = self.pending.windows(3).position(|b| b == b"\x1b[c") {
                self.pending.drain(index..index + 3);
                self.send("\x1b[?6c");
            }
            if let Some(index) = self
                .pending
                .windows(marker.len())
                .position(|bytes| bytes == marker.as_bytes())
            {
                let remainder = self.pending.split_off(index + marker.len());
                let output = String::from_utf8_lossy(&self.pending).into_owned();
                self.pending = remainder;
                return output;
            }
            let bytes = self
                .output
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|_| {
                    panic!(
                        "Timed out waiting for {marker:?}. Last output: {:?}",
                        String::from_utf8_lossy(&self.pending)
                    )
                });
            self.pending.extend(bytes);
            if self.pending.len() > 256_000 {
                self.pending.drain(..128_000);
            }
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.job.terminate();
        let _ = self.child.wait();
    }
}

#[test]
#[ignore = "Requires Git Bash, native Node and an installed AI Battery Codex bridge; no model call"]
fn git_windows_bridge_color_node_tty_and_codex_version() {
    use miniterm::prompt::{PromptFiles, PromptPreferences};
    let root = std::env::temp_dir().join(format!("MiniTerm bridge {}", uuid::Uuid::new_v4()));
    let files = PromptFiles::create(
        &root,
        &PromptPreferences {
            symbol: "BR>".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        miniterm::shell::codex_windows_bridge().is_some(),
        "Install the Windows AI Battery bridge to run this probe"
    );
    files.install_windows_bridge().unwrap();
    let mock = files.directory().join("tty-probe.cjs");
    std::fs::write(&mock, "if (!process.stdin.isTTY || !process.stdout.isTTY) process.exit(12); console.log('__NODE_TTY__'); console.log('__ARGS__'+JSON.stringify(process.argv.slice(2))); process.exit(7);").unwrap();
    let mut shell = Harness::spawn(
        git_test_profile(None),
        "bridge-test",
        Some(files.directory()),
    );
    shell.until("BR>");
    shell.send("HISTFILE=/dev/null; HISTSIZE=0; [[ -z ${NO_COLOR+x} && $TERM == xterm-256color && $COLORTERM == truecolor ]] && printf '__COLOR''_ENV__\\n'; [[ $(type -t codex) == function ]] && printf '__BRIDGE''_FUNCTION__\\n'\r");
    let output = shell.until("BR>");
    assert!(
        output.contains("__COLOR_ENV__") && output.contains("__BRIDGE_FUNCTION__"),
        "{output:?}"
    );
    let path = mock
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    shell.send(&format!(
        "node '{path}' '한글 x & ; $(literal)' 'quote\"value'; printf '__EXIT_%s__\\n' \"$?\"\r"
    ));
    let output = shell.until("BR>");
    assert!(
        output.contains("__NODE_TTY__")
            && output.contains("__EXIT_7__")
            && output.contains(r#"["한글 x & ; $(literal)","quote\"value"]"#),
        "{output:?}"
    );
    // Plain layout checks the real installed wrapper's launch without opening a quota HUD,
    // changing configuration, authenticating or requesting any model response.
    shell.send(
        "AI_BATTERY_WIN_LAYOUT=plain codex --version; printf '__CODEX_EXIT_%s__\\n' \"$?\"\r",
    );
    let output = shell.until("BR>");
    assert!(
        output.contains("codex-cli ") && output.contains("__CODEX_EXIT_0__"),
        "{output:?}"
    );
    std::fs::remove_file(mock).unwrap();
    drop(shell);
    drop(files);
    std::fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "Own IPC/config and localhost SSE provider; installed Codex/Node/AI Battery bridge required"]
fn git_codex_notify_rebinds_helper_and_preserves_original_callback() {
    use miniterm::{
        agent::{Broker, State},
        ipc,
        prompt::{PromptFiles, PromptPreferences},
    };
    use std::sync::{atomic::Ordering, Arc, Mutex};
    struct Environment(Vec<(&'static str, Option<std::ffi::OsString>)>);
    impl Drop for Environment {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }
    let root = std::env::current_dir()
        .unwrap()
        .join(format!("artifacts/codex-notify-{}", uuid::Uuid::new_v4()));
    let home = root.join("codex-home");
    let work = root.join("work");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir(&work).unwrap();
    let environment = Environment(
        [
            "MINITERM_DIAGNOSTICS",
            "MINITERM_INSTANCE_ID",
            "MINITERM_DATA_DIR",
        ]
        .into_iter()
        .map(|name| (name, std::env::var_os(name)))
        .collect(),
    );
    std::env::set_var("MINITERM_DIAGNOSTICS", "1");
    std::env::set_var("MINITERM_INSTANCE_ID", uuid::Uuid::new_v4().to_string());
    std::env::set_var("MINITERM_DATA_DIR", &root);
    let id = uuid::Uuid::new_v4().to_string();
    let broker = Arc::new(Mutex::new(Broker::default()));
    broker.lock().unwrap().register(id.clone());
    let server_broker = broker.clone();
    let server_id = id.clone();
    let ipc_server = ipc::serve(move |request| match request {
        ipc::Request::Sessions => ipc::Reply {
            ok: true,
            sessions: vec![server_id.clone()],
            ..Default::default()
        },
        ipc::Request::Event { event } => {
            if server_broker
                .lock()
                .unwrap()
                .receive(event, false)
                .is_some()
            {
                ipc::Reply::ok()
            } else {
                ipc::Reply::error("Unexpected or duplicate fixture event")
            }
        }
        _ => ipc::Reply::error("Fixture accepts only sessions/events"),
    })
    .unwrap();
    let response =
        responses_fixture::ResponsesFixture::start("__LOCAL_CODEX_NOTIFY_COMPLETED__ 한글");
    let callback = root.join("original-callback.cjs");
    let callback_log = root.join("callback.jsonl");
    let counter = format!(
        "require('node:fs').appendFileSync({},JSON.stringify(process.argv.slice(2))+'\\n');",
        serde_json::to_string(&callback_log).unwrap()
    );
    std::fs::write(&callback, counter).unwrap();
    let mut config: toml_edit::DocumentMut = format!("model='miniterm-notify-fixture'\nmodel_provider='fixture'\napproval_policy='never'\nsandbox_mode='read-only'\ncli_auth_credentials_store='file'\nweb_search='disabled'\nproject_root_markers=[]\n[model_providers.fixture]\nname='Local fixture'\nbase_url='{}'\nwire_api='responses'\nrequires_openai_auth=false\nsupports_websockets=false\nrequest_max_retries=0\nstream_max_retries=0\n", response.address).parse().unwrap();
    let mut notify = toml_edit::Array::new();
    for arg in [
        "node",
        callback.to_str().unwrap(),
        "literal 한글 & $(not-executed)",
    ] {
        notify.push(arg);
    }
    config["notify"] = toml_edit::value(notify);
    let config_path = home.join("config.toml");
    std::fs::write(&config_path, config.to_string()).unwrap();
    miniterm::integration::enable_codex(&home, &root.join("old-install/miniterm.exe")).unwrap();
    let config_before = std::fs::read(&config_path).unwrap();
    let backup_path = home.join("miniterm-notify.json");
    let backup_before = std::fs::read(&backup_path).unwrap();
    let prompt = PromptFiles::create(
        &root,
        &PromptPreferences {
            show_user: false,
            show_host: false,
            show_path: false,
            symbol: "NOTIFY>".into(),
            ..Default::default()
        },
    )
    .unwrap();
    prompt.install_windows_bridge().unwrap();
    let mut shell = Harness::spawn(git_test_profile(Some(work)), &id, Some(prompt.directory()));
    shell.until("NOTIFY>");
    let quote = |path: &std::path::Path| {
        format!(
            "'{}'",
            path.to_string_lossy()
                .replace('\\', "/")
                .replace('\'', "'\\''")
        )
    };
    let helper = std::path::Path::new(env!("CARGO_BIN_EXE_miniterm"));
    shell.send(&format!("HISTFILE=/dev/null; HISTSIZE=0; unset OPENAI_API_KEY CODEX_API_KEY OPENAI_BASE_URL; export CODEX_HOME={}; export MINITERM_NOTIFY_EXE={}; if __fixture_notify=$(command \"$MINITERM_NOTIFY_EXE\" codex-notify-override --no-daemon exec) && [[ -n $__fixture_notify ]]; then printf '__N''OTIFY_LOOKUP_OK__\\n'; fi; AI_BATTERY_WIN_LAYOUT=plain codex --no-daemon exec --strict-config --skip-git-repo-check --json 'Local notification fixture'; printf '__N''OTIFY_EXIT_%s__\\n' \"$?\"\r", quote(&home), quote(helper)));
    let output = shell.until("__NOTIFY_EXIT_0__");
    assert!(
        output.contains("__LOCAL_CODEX_NOTIFY_COMPLETED__"),
        "{output:?}"
    );
    assert!(
        output.contains("__NOTIFY_LOOKUP_OK__"),
        "Helper lookup did not rebind the owned link: {output:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if broker.lock().unwrap().sessions[&id].state == State::Complete {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Native Codex notify did not reach the namespaced broker. Output: {output:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    shell.until("NOTIFY>");
    shell.send("AI_BATTERY_WIN_LAYOUT=plain codex --no-daemon -c 'notify=[]' exec --strict-config --skip-git-repo-check --json 'Local disabled-notify fixture'; printf '__D''ISABLED_EXIT_%s__\\n' \"$?\"\r");
    let disabled = shell.until("__DISABLED_EXIT_0__");
    assert!(
        disabled.contains("__LOCAL_CODEX_NOTIFY_COMPLETED__"),
        "{disabled:?}"
    );
    shell.until("NOTIFY>");
    let mut profile = toml_edit::DocumentMut::new();
    let mut profile_notify = toml_edit::Array::new();
    for arg in ["node", callback.to_str().unwrap(), "profile callback"] {
        profile_notify.push(arg);
    }
    profile["notify"] = toml_edit::value(profile_notify);
    std::fs::write(home.join("custom.config.toml"), profile.to_string()).unwrap();
    shell.send("AI_BATTERY_WIN_LAYOUT=plain codex --no-daemon --profile custom exec --strict-config --skip-git-repo-check --json 'Local profile-notify fixture'; printf '__P''ROFILE_EXIT_%s__\\n' \"$?\"\r");
    let profile_output = shell.until("__PROFILE_EXIT_0__");
    assert!(
        profile_output.contains("__LOCAL_CODEX_NOTIFY_COMPLETED__"),
        "{profile_output:?}"
    );
    shell.until("NOTIFY>");
    std::fs::write(
        home.join("inherit.config.toml"),
        "model = 'miniterm-notify-fixture'\n",
    )
    .unwrap();
    shell.send("AI_BATTERY_WIN_LAYOUT=plain codex --no-daemon --profile inherit exec --strict-config --skip-git-repo-check --json 'Local inherited-notify fixture'; printf '__I''NHERIT_EXIT_%s__\\n' \"$?\"\r");
    let inherited_output = shell.until("__INHERIT_EXIT_0__");
    assert!(
        inherited_output.contains("__LOCAL_CODEX_NOTIFY_COMPLETED__"),
        "{inherited_output:?}"
    );
    shell.until("NOTIFY>");
    let bridge = quote(&prompt.directory().join("windows-bridge.bash"));
    shell.send(&format!("builtin unset -f codex; codex() {{ printf '__USER_FUNCTION_%s__\\n' \"$1\"; }}; builtin source {bridge}; codex literal; printf '__F''UNCTION_DONE__\\n'\r"));
    let user_function = shell.until("__FUNCTION_DONE__");
    assert!(
        user_function.contains("__USER_FUNCTION_literal__"),
        "{user_function:?}"
    );
    shell.until("NOTIFY>");
    shell.send(&format!("builtin unset -f codex; alias codex='printf \"__USER_ALIAS_%s__\\n\"'; builtin source {bridge}; printf '__A''LIAS_READY__\\n'\r"));
    shell.until("__ALIAS_READY__");
    shell.until("NOTIFY>");
    shell.send("codex literal; printf '__A''LIAS_DONE__\\n'\r");
    let user_alias = shell.until("__ALIAS_DONE__");
    assert!(
        user_alias.contains("__USER_ALIAS_literal__"),
        "{user_alias:?}"
    );
    let callbacks = std::fs::read_to_string(&callback_log).unwrap();
    let calls: Vec<serde_json::Value> = callbacks
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0][0], "literal 한글 & $(not-executed)");
    assert_eq!(calls[1][0], "profile callback");
    assert_eq!(calls[2][0], "literal 한글 & $(not-executed)");
    let payload: serde_json::Value = serde_json::from_str(calls[0][1].as_str().unwrap()).unwrap();
    assert_eq!(payload["type"], "agent-turn-complete");
    assert_eq!(
        payload["last-assistant-message"],
        "__LOCAL_CODEX_NOTIFY_COMPLETED__ 한글"
    );
    assert_eq!(broker.lock().unwrap().notices.len(), 2);
    assert_eq!(response.requests.load(Ordering::Acquire), 4);
    assert_eq!(std::fs::read(&config_path).unwrap(), config_before);
    assert_eq!(std::fs::read(&backup_path).unwrap(), backup_before);
    std::fs::write(root.join("verification.json"), serde_json::to_vec_pretty(&serde_json::json!({"nativeNotifyPassed":true,"explicitDisabledPreserved":true,"profileCallbackPreserved":true,"profileInheritancePassed":true,"userAliasAndFunctionPreserved":true,"callbackCount":calls.len(),"noticeCount":2,"configPreserved":true,"scope":"Native Codex + Windows bridge + helper + own IPC broker; localhost SSE fixture, no external model or GUI/toast."})).unwrap()).unwrap();
    println!("Codex notify fixture: {}", root.display());
    drop(shell);
    drop(prompt);
    drop(ipc_server);
    drop(environment);
}

#[test]
#[ignore = "Requires Git Bash; resize and immediate typeahead on a real ConPTY"]
fn bash_resize_immediate_typeahead_preserves_commands() {
    let mut shell = Harness::new(std::env::current_dir().unwrap());
    for n in 0..100 {
        shell
            .master
            .resize(pty::size(80 + n % 3, 24 + n % 20).unwrap())
            .unwrap();
        thread::sleep(Duration::from_millis(30));
        shell.send(&format!("printf '__RAW_''{n:02}_OK__\\n'\r"));
        shell.until(&format!("__RAW_{n:02}_OK__"));
    }
}

#[test]
#[ignore = "Requires Git Bash; managed prompt under rapid resize"]
fn bash_managed_prompt_resize_preserves_typeahead() {
    use miniterm::prompt::{PromptFiles, PromptPreferences};
    let root = std::env::temp_dir().join(format!("MiniTerm resize {}", uuid::Uuid::new_v4()));
    let files = PromptFiles::create(
        &root,
        &PromptPreferences {
            show_user: false,
            show_host: false,
            show_path: false,
            symbol: "R>".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut shell = Harness::spawn(
        git_test_profile(None),
        "managed-resize",
        Some(files.directory()),
    );
    shell.until("R>");
    for n in 0..100 {
        thread::sleep(Duration::from_millis(30));
        shell
            .master
            .resize(pty::size(80 + n % 3, 24 + n % 20).unwrap())
            .unwrap();
        thread::sleep(Duration::from_millis(150));
        shell.send(&format!("printf '__RAW_''{n:02}_OK__\\n'\r"));
        shell.until(&format!("__RAW_{n:02}_OK__"));
    }
    drop(shell);
    drop(files);
    std::fs::remove_dir(root).unwrap();
}

#[test]
#[ignore = "Production Session writer/parser under resize; requires Git Bash"]
fn git_session_input_after_resize_preserves_commands_and_ansi_styles() {
    check_session_input_and_styles(true, true);
}

#[test]
#[ignore = "Production Session typing and ANSI flags without resize; requires Git Bash"]
fn git_session_plain_input_and_ansi_styles() {
    check_session_input_and_styles(false, false);
}

#[test]
#[ignore = "Production Session vertical-only resize; requires Git Bash"]
fn git_session_vertical_resize_preserves_input_and_styles() {
    check_session_input_and_styles(false, true);
}

fn check_session_input_and_styles(resize_columns: bool, resize_rows: bool) {
    use alacritty_terminal::{
        grid::Dimensions,
        index::{Column, Line},
        term::cell::Flags,
    };
    use miniterm::{prompt::PromptPreferences, terminal::Session};
    let session = Session::start_with_id(
        git_test_profile(None),
        1000,
        egui::Context::default(),
        uuid::Uuid::new_v4().to_string(),
        Default::default(),
        Default::default(),
        PromptPreferences {
            show_user: false,
            show_host: false,
            show_path: false,
            symbol: "READY>".into(),
            ..Default::default()
        },
    )
    .unwrap();
    let wait_for = |marker: &str| {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let text = {
                let term = session.term.lock();
                (0..term.screen_lines())
                    .map(|line| {
                        (0..term.columns())
                            .map(|col| term.grid()[Line(line as i32)][Column(col)].c)
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            if text.contains(marker) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "Production Session lost command output: {marker}; fixture screen: {text:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    };
    wait_for("READY>");
    session
        .input
        .send(b"HISTFILE=/dev/null; HISTSIZE=0; printf '__HISTORY_''OFF__\\n'\r".to_vec())
        .unwrap();
    wait_for("__HISTORY_OFF__");
    for n in 0..30 {
        if resize_columns || resize_rows {
            session.resize(
                if resize_columns { 80 + n % 3 } else { 80 },
                if resize_rows { 24 + n % 20 } else { 24 },
                8,
                20,
            );
        }
        session
            .input
            .send(format!("printf '__SESSION_''{n:02}_OK__\\n'\r").into_bytes())
            .unwrap();
        wait_for(&format!("__SESSION_{n:02}_OK__"));
    }
    session.input.send(b"printf '\\033[2J\\033[H\\033[31mCOLOR\\033[1mBOLD\\033[3mITALIC\\033[0m __STYLE_''OK__\\n'\r".to_vec()).unwrap();
    wait_for("__STYLE_OK__");
    let term = session.term.lock();
    assert_eq!(term.grid()[Line(0)][Column(0)].c, 'C');
    assert!(term.grid()[Line(0)][Column(5)].flags.contains(Flags::BOLD));
    assert!(term.grid()[Line(0)][Column(9)]
        .flags
        .contains(Flags::ITALIC));
}

fn check_managed_prompt(profile: ShellProfile) {
    use miniterm::prompt::{PathStyle, PromptFiles, PromptPreferences};
    let root = std::env::temp_dir().join("MiniTerm prompt 한글 공백");
    let mut preferences = PromptPreferences {
        symbol: "M>".into(),
        ..Default::default()
    };
    let files = PromptFiles::create(&root, &preferences).unwrap();
    // Inject fixtures into our own generated startup, leaving the user's startup files intact.
    let bootstrap_path = files.directory().join("bootstrap.bash");
    let bootstrap = std::fs::read_to_string(&bootstrap_path).unwrap().replace("__miniterm_prompt_file=", concat!(
        "HISTFILE=/dev/null; HISTSIZE=0\n",
        "PS1='__ORIGINAL_PROMPT__ '\n",
        "shopt -u promptvars\n",
        "alias __miniterm_fixture_alias='printf ALIAS_OK'\n",
        "__miniterm_fixture_function() { printf FUNCTION_OK; }\n",
        "PROMPT_COMMAND=('__miniterm_fixture_status=$?; __miniterm_fixture_count=$(( ${__miniterm_fixture_count:-0} + 1 ))')\n",
        "__miniterm_prompt_file="
    ));
    std::fs::write(&bootstrap_path, bootstrap).unwrap();
    let mut shell = Harness::spawn(profile, "managed-prompt-test", Some(files.directory()));
    let first_prompt = shell.until("M>");
    assert!(
        !first_prompt.contains("MINGW64"),
        "Git's multi-line prompt must be replaced: {first_prompt:?}"
    );
    shell.send("shopt -q login_shell && [[ $__miniterm_fixture_count == 1 && -t 0 && -t 1 ]] && [[ $PS1 != *'\\n'* && $PS1 != *$'\\n'* ]] && printf '__LOGIN''_ONE_LINE__\\n'; __miniterm_fixture_alias; __miniterm_fixture_function; false\r");
    let output = shell.until("M>");
    assert!(
        output.contains("__LOGIN_ONE_LINE__")
            && output.contains("ALIAS_OK")
            && output.contains("FUNCTION_OK"),
        "{output:?}"
    );
    shell.send("[[ $__miniterm_fixture_status == 1 ]] && printf '__STATUS''_OK__\\n'\r");
    let output = shell.until("M>");
    assert!(
        output.contains("__STATUS_OK__"),
        "Existing PROMPT_COMMAND must retain exit status: {output:?}"
    );

    // A foreground read emulates a TUI's input. Updating preferences must not feed it a command.
    shell.send("IFS= read -r __miniterm_fixture_input; printf '__READ_%s__\\n' \"$__miniterm_fixture_input\"\r");
    thread::sleep(Duration::from_millis(150));
    preferences.show_host = false;
    preferences.user_label = "$(printf INJECTION) `printf INJECTION` \\n 한글".into();
    preferences.path_style = PathStyle::Folder;
    preferences.symbol = "N>".into();
    files.update(&preferences).unwrap();
    thread::sleep(Duration::from_millis(100));
    shell.send("literal-input\r");
    let output = shell.until("N>");
    assert!(
        output.contains("__READ_literal-input__"),
        "Prompt update must preserve foreground input: {output:?}"
    );
    assert!(
        output.contains("$(printf INJECTION) `printf INJECTION` \\n 한글"),
        "Prompt aliases must be literal text: {output:?}"
    );

    preferences.enabled = false;
    files.update(&preferences).unwrap();
    shell.send("printf '__DISABLE''_OK__\\n'\r");
    let output = shell.until("__ORIGINAL_PROMPT__ ");
    assert!(output.contains("__DISABLE_OK__"));
    shell.send("! shopt -q promptvars && printf '__RESTORE''_OPTION__\\n'\r");
    let output = shell.until("__ORIGINAL_PROMPT__ ");
    assert!(output.contains("__RESTORE_OPTION__"));
    preferences.enabled = true;
    preferences.show_user = false;
    preferences.show_path = false;
    files.update(&preferences).unwrap();
    shell.send("printf '__ENABLE''_OK__\\n'\r");
    let output = shell.until("N>");
    assert!(
        output.contains("__ENABLE_OK__") && !output.contains("$(printf INJECTION)"),
        "{output:?}"
    );
    // Benchmark the managed hook, including its tiny file read, without overriding PS1.
    let mut samples = Vec::new();
    for index in 0..100 {
        let start = Instant::now();
        shell.send(&format!("printf '__MANAGED''_{index}__\\n'\r"));
        shell.until(&format!("__MANAGED_{index}__"));
        shell.until("N>");
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "Managed prompt PTY round-trip: median {:.3}ms, p95 {:.3}ms (100 samples; excludes GUI)",
        samples[49], samples[94]
    );
    drop(shell);
    let directory = files.directory().to_owned();
    drop(files);
    assert!(
        !directory.exists(),
        "The session must clean up its owned prompt files"
    );
    let _ = std::fs::remove_dir(root);
}

#[test]
#[ignore = "Requires Git for Windows and a real ConPTY"]
fn git_managed_prompt_login_live_changes_literals_and_foreground_input() {
    check_managed_prompt(git_test_profile(None));
}

#[test]
#[ignore = "Requires MINITERM_WSL_TEST_DISTRO and a real WSL2 / ConPTY"]
fn wsl_managed_prompt_login_live_changes_literals_and_foreground_input() {
    check_managed_prompt(ShellProfile::Wsl {
        distribution: std::env::var("MINITERM_WSL_TEST_DISTRO")
            .expect("Set MINITERM_WSL_TEST_DISTRO"),
        user: None,
        directory: "~".into(),
    });
}

#[test]
#[ignore = "Requires installed Git for Windows; runs a real interactive ConPTY session"]
fn bash_state_unicode_tty_resize_and_interrupt() {
    let directory = std::env::temp_dir().join(format!("MiniTerm 공백 {}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    {
        let mut shell = Harness::new(directory.clone());
        let pid = shell.child.process_id();
        shell.send("[[ -t 0 && -t 1 ]] && printf '__TTY''_OK__\\n'; [[ $PWD == *'MiniTerm 공백'* ]] && printf '__CWD''_OK__\\n'; export MINI_STATE=kept; printf '__SET''_OK__\\n'\r");
        let output = shell.until("__SET_OK__");
        assert!(output.contains("__TTY_OK__"), "{output:?}");
        assert!(output.contains("__CWD_OK__"), "{output:?}");
        shell.until("__MINITERM_PROMPT__ ");
        shell.master.resize(pty::size(120, 30).unwrap()).unwrap();
        shell.send("[[ $MINI_STATE == kept ]] && printf '__STATE''_OK__\\n'; printf '%s\\n' 한글 | cat; printf '__PIPE''_OK__\\n'\r");
        let output = shell.until("__PIPE_OK__");
        assert!(output.contains("__STATE_OK__"));
        assert!(output.contains("한글"));
        assert_eq!(shell.child.process_id(), pid);
        assert_eq!(shell.master.get_size().unwrap().cols, 120);
        shell.until("__MINITERM_PROMPT__ ");
        shell.send("sleep 20\r");
        thread::sleep(Duration::from_millis(150));
        shell.send("\x03");
        // Wait for the new prompt: terminals flush queued typeahead when processing Ctrl+C.
        shell.until("__MINITERM_PROMPT__ ");
        shell.send("printf '__CTRL''_C_OK__\\n'\r");
        shell.until("__CTRL_C_OK__");
        shell.until("__MINITERM_PROMPT__ ");
        shell.send("sleep 20\r");
        thread::sleep(Duration::from_millis(150));
        shell.send("\x1a");
        let output = shell.until("__MINITERM_PROMPT__ ");
        assert!(
            output.contains("Stopped"),
            "Ctrl+Z must suspend the foreground job: {output:?}"
        );
        shell.send("jobs; bg; fg\r");
        thread::sleep(Duration::from_millis(150));
        shell.send("\x03");
        shell.until("__MINITERM_PROMPT__ ");
        shell.send("printf '\\033[?1049hALT\\033[?1049l'; printf '__ALT''_OK__\\n'\r");
        shell.until("__ALT_OK__");
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::remove_dir(&directory) {
            Ok(()) => break,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Err(error) => panic!("Temporary cwd was not released after shell exit: {error}"),
        }
    }
}

#[test]
#[ignore = "Local baseline for PTY round-trip latency; not a GUI/input-to-pixel benchmark"]
fn conpty_round_trip_100_samples() {
    let mut shell = Harness::new(std::env::current_dir().unwrap());
    let mut samples = Vec::new();
    for index in 0..100 {
        let marker = format!("__ROUND_TRIP_{index}__");
        let start = Instant::now();
        shell.send(&format!("printf '__ROUND''_TRIP_{index}__\\n'\r"));
        shell.until(&marker);
        shell.until("__MINITERM_PROMPT__ ");
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "ConPTY/Bash round-trip: n=100 median={:.3}ms p95={:.3}ms min={:.3}ms max={:.3}ms",
        samples[49], samples[94], samples[0], samples[99]
    );
}

#[test]
#[ignore = "Requires explicitly selected WSL2 distro in MINITERM_WSL_TEST_DISTRO; starts that distro"]
fn wsl_home_environment_resize_job_control_and_windows_interop() {
    let distribution = std::env::var("MINITERM_WSL_TEST_DISTRO")
        .expect("Set MINITERM_WSL_TEST_DISTRO to a test distribution");
    let id = uuid::Uuid::new_v4().to_string();
    let mut shell = Harness::profile(
        ShellProfile::Wsl {
            distribution,
            user: None,
            directory: "~".into(),
        },
        &id,
    );
    let pid = shell.child.process_id();
    shell.send(&format!("[[ $PWD == \"$HOME\" && -t 0 && -t 1 ]] && printf '__HOME''_TTY_OK__\\n'; [[ $MINITERM_SESSION_ID == '{id}' && -f $MINITERM_NOTIFY_EXE && $MINITERM_NOTIFY_EXE == /mnt/* ]] && printf '__ENV''_OK__\\n'; printf '%s\\n' 한글; stat -f -c 'filesystem=%T' .; printf '__PROBE''_OK__\\n'\r"));
    let output = shell.until("__PROBE_OK__");
    assert!(output.contains("__HOME_TTY_OK__"), "{output:?}");
    assert!(output.contains("__ENV_OK__"), "{output:?}");
    assert!(output.contains("한글"), "{output:?}");
    assert!(
        output.contains("filesystem=ext2/ext3"),
        "WSL home should use the Linux filesystem: {output:?}"
    );
    shell.until("__MINITERM_PROMPT__ ");
    shell.send("\"$MINITERM_NOTIFY_EXE\" --list; [[ $? == 0 ]] && printf '__WIN32''_INTEROP_OK__\\n'; printf '__INTEROP''_DONE__\\n'\r");
    let output = shell.until("__INTEROP_DONE__");
    assert!(
        output.contains("__WIN32_INTEROP_OK__"),
        "Windows helper invocation through WSL interop: {output:?}"
    );
    shell.until("__MINITERM_PROMPT__ ");
    shell.master.resize(pty::size(120, 30).unwrap()).unwrap();
    shell.send("printf 'size='; stty size; printf '__SIZE''_OK__\\n'\r");
    let output = shell.until("__SIZE_OK__");
    assert!(output.contains("30 120"), "{output:?}");
    assert_eq!(shell.child.process_id(), pid);
    shell.until("__MINITERM_PROMPT__ ");
    shell.send("sleep 20\r");
    thread::sleep(Duration::from_millis(150));
    shell.send("\x1a");
    let output = shell.until("__MINITERM_PROMPT__ ");
    assert!(output.contains("Stopped"), "{output:?}");
    shell.send("jobs; bg; fg\r");
    thread::sleep(Duration::from_millis(150));
    shell.send("\x03");
    shell.until("__MINITERM_PROMPT__ ");
    shell.send("printf '__INTERRUPT''_OK__\\n'\r");
    shell.until("__INTERRUPT_OK__");
}
