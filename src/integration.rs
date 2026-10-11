//! Opt-in integration editing. Own commands are recorded and removed by exact match.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml_edit::{value, Array, DocumentMut};

#[derive(Serialize, Deserialize)]
struct CodexBackup {
    original: Option<Vec<String>>,
    installed: Vec<String>,
}
fn read(path: &Path) -> Result<String, String> {
    if path.exists() {
        fs::read_to_string(path).map_err(|e| e.to_string())
    } else {
        Ok(String::new())
    }
}
fn write(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = path.with_extension("miniterm.tmp");
    fs::write(&temp, contents).map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}
fn preserve_backup(path: &Path) -> Result<(), String> {
    let backup = path.with_extension("before-miniterm");
    if path.exists() && !backup.exists() {
        fs::copy(path, backup).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn codex_home() -> Result<PathBuf, String> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".codex")))
        .ok_or("사용자 홈을 찾을 수 없습니다.".into())
}
pub fn claude_settings() -> Result<PathBuf, String> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".claude")))
        .map(|path| path.join("settings.json"))
        .ok_or("사용자 홈을 찾을 수 없습니다.".into())
}
fn array(document: &DocumentMut) -> Result<Option<Vec<String>>, String> {
    match document.get("notify") {
        None => Ok(None),
        Some(item) => item
            .as_array()
            .ok_or("기존 Codex notify 형식을 확인하세요.")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or("기존 Codex notify 인자를 확인하세요.".into())
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
    }
}
fn set_notify(document: &mut DocumentMut, args: &[String]) {
    let mut entries = Array::new();
    for arg in args {
        entries.push(arg.as_str());
    }
    document["notify"] = value(entries);
}

/// Rebind an already enabled, exactly owned link for this invocation only.
/// The recorded callback chain and the user's files remain unchanged.
pub fn session_codex_notify(
    home: &Path,
    helper: &Path,
    args: &[String],
) -> Result<Option<String>, String> {
    if codex_has_notification_override(args) {
        return Ok(None);
    }
    let backup = home.join("miniterm-notify.json");
    if !backup.is_file() {
        return Ok(None);
    }
    let record: CodexBackup = serde_json::from_str(&read(&backup)?).map_err(|e| e.to_string())?;
    let mut config = read(&home.join("config.toml"))?
        .parse::<DocumentMut>()
        .map_err(|e| e.to_string())?;
    if let Some(name) = codex_profile(args) {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Ok(None);
        }
        let path = home.join(format!("{name}.config.toml"));
        if !path.is_file() {
            return Ok(None);
        }
        let profile = read(&path)?
            .parse::<DocumentMut>()
            .map_err(|e| e.to_string())?;
        if let Some(notify) = array(&profile)? {
            set_notify(&mut config, &notify);
        }
    }
    if array(&config)?.as_ref() != Some(&record.installed)
        || record.installed.len() != 5
        || record.installed[1..4] != ["notify", "codex", "--chain-file"]
        || Path::new(&record.installed[4]) != backup
        || Path::new(&record.installed[0]) == helper
    {
        return Ok(None);
    }
    let mut args = record.installed;
    args[0] = helper.to_string_lossy().into_owned();
    serde_json::to_string(&args)
        .map(|args| Some(format!("notify={args}")))
        .map_err(|e| e.to_string())
}

/// Respect explicit CLI notify overrides. Profiles are resolved separately so
/// a model-only profile can still inherit its existing MiniTerm connection.
pub fn codex_has_notification_override(args: &[String]) -> bool {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        let config = if matches!(arg.as_str(), "-c" | "--config") {
            args.next().map(String::as_str)
        } else {
            arg.strip_prefix("--config=")
                .or_else(|| arg.strip_prefix("-c"))
        };
        if config
            .and_then(|s| s.split_once('='))
            .is_some_and(|(key, _)| key.trim() == "notify")
        {
            return true;
        }
    }
    false
}
fn codex_profile(args: &[String]) -> Option<&str> {
    let mut args = args.iter();
    let mut profile = None;
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if matches!(arg.as_str(), "-c" | "--config") {
            args.next();
            continue;
        }
        if matches!(arg.as_str(), "-p" | "--profile") {
            profile = Some(args.next().map_or("", String::as_str));
        } else if let Some(name) = arg
            .strip_prefix("--profile=")
            .or_else(|| arg.strip_prefix("-p"))
        {
            profile = Some(name);
        }
    }
    profile
}
pub fn enable_codex(home: &Path, helper: &Path) -> Result<(), String> {
    let config = home.join("config.toml");
    let backup = home.join("miniterm-notify.json");
    let mut document = read(&config)?
        .parse::<DocumentMut>()
        .map_err(|e| e.to_string())?;
    let original = array(&document)?;
    let installed = vec![
        helper.display().to_string(),
        "notify".into(),
        "codex".into(),
        "--chain-file".into(),
        backup.display().to_string(),
    ];
    if original.as_ref() == Some(&installed) {
        return Ok(());
    }
    if backup.exists() {
        return Err(
            "Codex 연동 기록이 이미 있습니다. 기존 연동을 해제한 뒤 다시 연결하세요.".into(),
        );
    }
    if original
        .as_ref()
        .is_some_and(|args| args.iter().any(|s| s == "--chain-file"))
    {
        return Err("기존 notify 연결을 먼저 확인하세요.".into());
    }
    preserve_backup(&config)?;
    write(
        &backup,
        &serde_json::to_vec_pretty(&CodexBackup {
            original,
            installed: installed.clone(),
        })
        .map_err(|e| e.to_string())?,
    )?;
    set_notify(&mut document, &installed);
    write(&config, document.to_string().as_bytes())
}
pub fn disable_codex(home: &Path, helper: &Path) -> Result<(), String> {
    let config = home.join("config.toml");
    let backup = home.join("miniterm-notify.json");
    if !backup.exists() {
        return Ok(());
    }
    let record: CodexBackup = serde_json::from_str(&read(&backup)?).map_err(|e| e.to_string())?;
    if record.installed.first().map(PathBuf::from).as_deref() != Some(helper) {
        return Ok(());
    }
    let mut document = read(&config)?
        .parse::<DocumentMut>()
        .map_err(|e| e.to_string())?;
    if array(&document)?.as_ref() != Some(&record.installed) {
        return Err("Codex notify는 연결 후 변경되었습니다. 현재 설정을 보존했습니다.".into());
    }
    if let Some(original) = record.original {
        set_notify(&mut document, &original);
    } else {
        document.remove("notify");
    }
    write(&config, document.to_string().as_bytes())?;
    fs::remove_file(backup).map_err(|e| e.to_string())
}
/// Existing notify receives the original JSON, including outside MiniTerm sessions.
/// It is never interpreted as shell code.
pub fn chain_codex(path: &Path, payload: &str) -> Result<(), String> {
    let record: CodexBackup = serde_json::from_str(&read(path)?).map_err(|e| e.to_string())?;
    if let Some(args) = record.original.filter(|args| !args.is_empty()) {
        let mut command = Command::new(&args[0]);
        command.args(&args[1..]).arg(payload);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        // The forwarding helper inherits Codex's notify lifecycle. No GUI thread waits here.
        command.status().map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn claude_command(helper: &Path) -> Result<String, String> {
    let path = helper.display().to_string();
    if path.contains(['\'', '\n', '\r']) {
        return Err("연동 도우미 경로에 지원하지 않는 문자가 있습니다.".into());
    }
    // Claude command hooks use a shell. Single quotes preserve spaces, $, backticks, and Unicode.
    Ok(format!("'{}' notify claude", path.replace('\\', "/")))
}
const CLAUDE_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PermissionRequest",
    "PostToolUse",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

#[derive(Serialize, Deserialize)]
struct OwnedLink {
    agent: crate::agent::Agent,
    config: PathBuf,
    helper: PathBuf,
}
fn manifest() -> Result<PathBuf, String> {
    Ok(crate::settings::data_directory()?.join("integrations.json"))
}
pub fn configure_user(
    agent: crate::agent::Agent,
    helper: &Path,
    enabled: bool,
) -> Result<(), String> {
    let config = match agent {
        crate::agent::Agent::Codex => codex_home()?.join("config.toml"),
        crate::agent::Agent::Claude => claude_settings()?,
    };
    let path = manifest()?;
    let mut links: Vec<OwnedLink> = if path.exists() {
        serde_json::from_str(&read(&path)?).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };
    if links.len() > 32 {
        return Err("에이전트 연결 기록을 확인하세요.".into());
    }
    match agent {
        crate::agent::Agent::Codex => {
            let home = config.parent().ok_or("Codex 설정 폴더를 확인하세요.")?;
            if enabled {
                enable_codex(home, helper)?;
            } else {
                disable_codex(home, helper)?;
            }
        }
        crate::agent::Agent::Claude => configure_claude(&config, helper, enabled)?,
    }
    links.retain(|link| !(link.agent == agent && link.config == config && link.helper == helper));
    if enabled {
        links.push(OwnedLink {
            agent,
            config,
            helper: helper.into(),
        });
    }
    write(
        &path,
        &serde_json::to_vec_pretty(&links).map_err(|e| e.to_string())?,
    )
}
pub fn cleanup(helper: &Path) -> Result<(), String> {
    let path = manifest()?;
    if !path.exists() {
        return Ok(());
    }
    let links: Vec<OwnedLink> = serde_json::from_str(&read(&path)?).map_err(|e| e.to_string())?;
    if links.len() > 32 {
        return Err("에이전트 연결 기록을 확인하세요.".into());
    }
    let mut retained = Vec::new();
    let mut errors = Vec::new();
    for link in links {
        if link.helper != helper {
            retained.push(link);
            continue;
        }
        let result = match link.agent {
            crate::agent::Agent::Codex => link
                .config
                .parent()
                .ok_or("Codex 설정 폴더를 확인하세요.".to_string())
                .and_then(|home| disable_codex(home, helper)),
            crate::agent::Agent::Claude => configure_claude(&link.config, helper, false),
        };
        if let Err(error) = result {
            errors.push(error);
            retained.push(link);
        }
    }
    write(
        &path,
        &serde_json::to_vec_pretty(&retained).map_err(|e| e.to_string())?,
    )?;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}
pub fn configure_claude(path: &Path, helper: &Path, enabled: bool) -> Result<(), String> {
    let contents = read(path)?;
    let mut document: Value = if contents.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&contents).map_err(|e| e.to_string())?
    };
    let object = document
        .as_object_mut()
        .ok_or("Claude 설정은 JSON 객체여야 합니다.")?;
    let command = claude_command(helper)?;
    if !enabled && !object.contains_key("hooks") {
        return Ok(());
    }
    let hooks = object
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("Claude hooks 형식을 확인하세요.")?;
    for event in CLAUDE_EVENTS {
        if !enabled && !hooks.contains_key(*event) {
            continue;
        }
        let entries = hooks
            .entry(*event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or("Claude 훅 목록 형식을 확인하세요.")?;
        for entry in entries.iter_mut() {
            if let Some(handlers) = entry.get_mut("hooks").and_then(Value::as_array_mut) {
                handlers.retain(|h| h.get("command").and_then(Value::as_str) != Some(&command));
            }
        }
        entries.retain(|entry| {
            !entry
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        });
        if enabled {
            let matcher = if *event == "Notification" {
                "permission_prompt|idle_prompt|elicitation_dialog"
            } else {
                ""
            };
            entries.push(json!({ "matcher": matcher, "hooks": [{ "type": "command", "command": command, "timeout": 5 }] }));
        }
    }
    hooks.retain(|_, entries| !entries.as_array().is_some_and(Vec::is_empty));
    if hooks.is_empty() {
        object.remove("hooks");
    }
    preserve_backup(path)?;
    write(
        path,
        &serde_json::to_vec_pretty(&document).map_err(|e| e.to_string())?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp() -> PathBuf {
        let path = std::env::temp_dir().join(format!("miniterm-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn session_notify_rebinds_only_an_exact_owned_link_without_editing_files() {
        let home = temp();
        let old = Path::new("C:/Installed MiniTerm/miniterm.exe");
        let current = Path::new("C:/한글 $() ' Preview/miniterm.exe");
        let config = home.join("config.toml");
        assert!(session_codex_notify(&home, current, &[]).unwrap().is_none());
        fs::write(
            &config,
            "# user comment\nnotify = ['original', 'literal argument']\n",
        )
        .unwrap();
        enable_codex(&home, old).unwrap();
        let config_before = fs::read(&config).unwrap();
        let backup = home.join("miniterm-notify.json");
        let backup_before = fs::read(&backup).unwrap();
        let override_config = session_codex_notify(&home, current, &[]).unwrap().unwrap();
        let parsed: DocumentMut = override_config.parse().unwrap();
        let rebound = array(&parsed).unwrap().unwrap();
        assert_eq!(rebound[0], current.to_string_lossy());
        assert_eq!(
            &rebound[1..],
            ["notify", "codex", "--chain-file", backup.to_str().unwrap()]
        );
        assert_eq!(fs::read(&config).unwrap(), config_before);
        assert_eq!(fs::read(&backup).unwrap(), backup_before);
        assert!(session_codex_notify(&home, old, &[]).unwrap().is_none());
        fs::write(&config, "notify = ['later user callback']\n").unwrap();
        assert!(session_codex_notify(&home, current, &[]).unwrap().is_none());
        fs::write(&config, config_before).unwrap();
        let mut record: CodexBackup = serde_json::from_slice(&backup_before).unwrap();
        record.installed[4] = "different-chain.json".into();
        fs::write(&backup, serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(session_codex_notify(&home, current, &[]).unwrap().is_none());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn explicit_codex_notification_policy_is_preserved() {
        let check = |args: &[&str]| {
            codex_has_notification_override(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        };
        for args in [
            vec!["-c", "notify=[]"],
            vec!["--config", " notify = []"],
            vec!["--config=notify=['custom']"],
            vec!["-cnotify=[]"],
        ] {
            assert!(check(&args), "{args:?}");
        }
        for args in [
            vec!["--no-daemon", "exec", "literal prompt"],
            vec!["-c", "model='fixture'"],
            vec!["--config=tui.notifications=false"],
            vec!["--", "-pfoo"],
            vec!["--config", "notify_extra=[]"],
            vec!["-p", "custom"],
            vec!["--profile=custom"],
            vec!["-pcustom"],
        ] {
            assert!(!check(&args), "{args:?}");
        }
    }
    #[test]
    fn profile_notify_policy_overlays_the_owned_base_connection() {
        let home = temp();
        let old = Path::new("C:/Installed MiniTerm/miniterm.exe");
        let current = Path::new("C:/Preview/miniterm.exe");
        enable_codex(&home, old).unwrap();
        let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let inherited = home.join("inherit.config.toml");
        fs::write(&inherited, "model = 'fixture'\n").unwrap();
        for values in [
            vec!["-p", "inherit"],
            vec!["-pinherit"],
            vec!["--profile=inherit"],
        ] {
            assert!(session_codex_notify(&home, current, &args(&values))
                .unwrap()
                .is_some());
        }
        assert!(
            session_codex_notify(&home, current, &args(&["-p", "missing"]))
                .unwrap()
                .is_none()
        );
        assert!(
            session_codex_notify(&home, current, &args(&["-p", "../inherit"]))
                .unwrap()
                .is_none()
        );
        for contents in ["notify = []\n", "notify = ['user callback']\n"] {
            fs::write(&inherited, contents).unwrap();
            assert!(
                session_codex_notify(&home, current, &args(&["-p", "inherit"]))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(read(&inherited).unwrap(), contents);
        }
        let owned = read(&home.join("config.toml")).unwrap();
        fs::write(&inherited, &owned).unwrap();
        assert!(
            session_codex_notify(&home, current, &args(&["-p", "inherit"]))
                .unwrap()
                .is_some()
        );
        assert!(
            session_codex_notify(&home, current, &args(&["-p", "inherit", "-c", "notify=[]"]))
                .unwrap()
                .is_none()
        );
        assert_eq!(read(&inherited).unwrap(), owned);
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn preserves_codex_notify_and_refuses_to_overwrite_later_edits() {
        let home = temp();
        let config = home.join("config.toml");
        let helper = Path::new("C:/MiniTerm/miniterm.exe");
        fs::write(
            &config,
            "# keep comment\nmodel = 'test'\nnotify = ['existing', 'arg']\n[features]\na = true\n",
        )
        .unwrap();
        enable_codex(&home, helper).unwrap();
        enable_codex(&home, helper).unwrap();
        assert!(read(&config).unwrap().contains("# keep comment"));
        disable_codex(&home, helper).unwrap();
        let restored = read(&config).unwrap().parse::<DocumentMut>().unwrap();
        assert_eq!(array(&restored).unwrap().unwrap(), ["existing", "arg"]);
        enable_codex(&home, helper).unwrap();
        let mut changed = read(&config).unwrap().parse::<DocumentMut>().unwrap();
        set_notify(&mut changed, &["new-user-command".into()]);
        fs::write(&config, changed.to_string()).unwrap();
        assert!(disable_codex(&home, helper).is_err());
        assert_eq!(
            array(&read(&config).unwrap().parse().unwrap())
                .unwrap()
                .unwrap(),
            ["new-user-command"]
        );
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn claude_add_remove_is_idempotent_and_keeps_other_hooks() {
        let home = temp();
        let config = home.join("settings.json");
        let helper = Path::new("C:/한글 Path/miniterm.exe");
        let original = json!({ "theme": "dark", "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "existing" }] }] } });
        fs::write(&config, original.to_string()).unwrap();
        configure_claude(&config, helper, true).unwrap();
        configure_claude(&config, helper, true).unwrap();
        let connected: Value = serde_json::from_str(&read(&config).unwrap()).unwrap();
        assert_eq!(connected["hooks"]["Stop"].as_array().unwrap().len(), 2);
        configure_claude(&config, helper, false).unwrap();
        let disconnected: Value = serde_json::from_str(&read(&config).unwrap()).unwrap();
        assert_eq!(original, disconnected);
        fs::remove_dir_all(home).unwrap();
    }
}
