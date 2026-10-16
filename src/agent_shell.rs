//! Invocation-only Bash guidance. Ask Codex to resolve its own configuration so
//! trusted project layers, profiles and existing developer instructions survive.
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

const BASH_INSTRUCTIONS: &str = "<miniterm_shell>\nThis session runs in MiniTerm Git Bash on Windows. For filesystem, search, build and script commands use Bash syntax (ls, rg, pipelines, heredocs and Bash scripts). Use exec_command with shell=\"bash\" and login=false for every Bash command; omitting shell selects Windows PowerShell. Bash is available on this process's PATH. Use Windows paths for the tool's workdir and Bash paths inside scripts. Keep script text intact, including newlines, quotes and $ expressions. Use PowerShell only for a task that explicitly requires it. Follow the existing permission and approval policy.\n</miniterm_shell>";

/// Only forward configuration flags, never a task, authentication or a command.
fn lookup_arguments(args: &[String]) -> Option<(Vec<String>, String, Option<String>)> {
    let mut forwarded = Vec::new();
    let mut cwd = std::env::current_dir().ok()?.to_string_lossy().into_owned();
    let mut positional_seen = false;
    let mut profile = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if matches!(
            arg.as_str(),
            "--help" | "-h" | "--version" | "-V" | "--remote"
        ) || arg.starts_with("--remote=")
        {
            return None;
        }
        if !positional_seen
            && matches!(
                arg.as_str(),
                "login"
                    | "logout"
                    | "mcp"
                    | "plugin"
                    | "app-server"
                    | "app"
                    | "completion"
                    | "update"
                    | "doctor"
                    | "sandbox"
                    | "debug"
                    | "apply"
                    | "cloud"
                    | "exec-server"
                    | "remote-control"
                    | "agents"
                    | "queue"
                    | "archive"
                    | "delete"
                    | "migrate-rollouts"
                    | "unarchive"
                    | "features"
                    | "help"
            )
        {
            return None;
        }
        if !arg.starts_with('-') {
            positional_seen = true;
        }
        if matches!(arg.as_str(), "-C" | "--cd") {
            cwd = std::path::absolute(args.next()?)
                .ok()?
                .to_string_lossy()
                .into_owned();
        } else if let Some(value) = arg
            .strip_prefix("--cd=")
            .or_else(|| arg.strip_prefix("-C").filter(|s| !s.is_empty()))
        {
            cwd = std::path::absolute(value)
                .ok()?
                .to_string_lossy()
                .into_owned();
        } else if matches!(arg.as_str(), "-p" | "--profile") {
            profile = Some(args.next()?.clone());
        } else if let Some(name) = arg
            .strip_prefix("--profile=")
            .or_else(|| arg.strip_prefix("-p").filter(|s| !s.is_empty()))
        {
            profile = Some(name.to_owned());
        } else if matches!(arg.as_str(), "-c" | "--config" | "--enable" | "--disable") {
            forwarded.extend([arg.clone(), args.next()?.clone()]);
        } else if arg.starts_with("--config=")
            || arg.starts_with("--enable=")
            || arg.starts_with("--disable=")
            || (arg.starts_with("-c") && arg.len() > 2)
        {
            forwarded.push(arg.clone());
        } else if matches!(
            arg.as_str(),
            "-m" | "--model"
                | "-s"
                | "--sandbox"
                | "-a"
                | "--ask-for-approval"
                | "-i"
                | "--image"
                | "--add-dir"
                | "--local-provider"
                | "--remote-auth-token-env"
        ) {
            args.next()?;
        }
    }
    Some((forwarded, cwd, profile))
}

fn apply_profile(response: &mut Value, profile: &str) -> Result<(), String> {
    if profile.is_empty()
        || !profile
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err("Codex profile name is invalid".into());
    }
    // app-server currently rejects --profile. The v2 user profile sits between
    // user and project layers. Read only that trusted user file, retaining the
    // project/CLI/managed value already resolved by config/read when higher.
    let path = crate::integration::codex_home()?.join(format!("{profile}.config.toml"));
    let text = std::fs::read_to_string(path).map_err(|_| "Codex profile could not be read")?;
    let document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "Codex profile is invalid")?;
    if ["projects", "project_root_markers", "profile", "profiles"]
        .iter()
        .any(|key| document.contains_key(key))
    {
        return Err("Bash guidance cannot resolve this profile's project/legacy layering".into());
    }
    let Some(item) = document.get("developer_instructions") else {
        return Ok(());
    };
    let instruction = item
        .as_str()
        .ok_or("Codex profile instructions are invalid")?;
    let origin = response
        .pointer("/origins/developer_instructions/name/type")
        .and_then(Value::as_str);
    if origin.is_none()
        || matches!(
            origin,
            Some("user" | "system" | "packagedDefaults" | "mdm" | "enterpriseManaged")
        )
    {
        response["config"]["developer_instructions"] = json!(instruction);
    }
    Ok(())
}

fn merged_instruction(config: &Value) -> Result<String, String> {
    let existing = match config.get("developer_instructions") {
        Some(Value::String(text)) => text.as_str(),
        Some(Value::Null) => "",
        _ => return Err("Codex did not return its developer instruction setting".into()),
    };
    let instructions = if existing.contains("<miniterm_shell>") {
        existing.to_owned()
    } else if existing.is_empty() {
        BASH_INSTRUCTIONS.to_owned()
    } else {
        format!("{existing}\n\n{BASH_INSTRUCTIONS}")
    };
    // JSON string escaping is also valid for these TOML basic strings. The
    // output stays on one line and Bash passes it as one argument, without eval.
    Ok(format!(
        "developer_instructions={}",
        serde_json::to_string(&instructions).map_err(|e| e.to_string())?
    ))
}

/// launch is an EXE, or Node plus the original launcher script. It is never
/// evaluated by a shell. Only a short-lived stdio config/read server is started.
pub fn codex_config(launch: &[String], args: &[String]) -> Result<Option<String>, String> {
    let Some((flags, cwd, profile)) = lookup_arguments(args) else {
        return Ok(None);
    };
    let executable = launch.first().ok_or("Codex launcher is missing")?;
    let mut command = Command::new(executable);
    command.args(&launch[1..]).args(flags).arg("app-server");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Configuration lookup must not open a console window in the GUI app.
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
    let mut child = command
        .spawn()
        .map_err(|_| "Codex configuration lookup could not start")?;
    use std::os::windows::io::AsRawHandle;
    let job = crate::native::ProcessJob::assign(child.as_raw_handle() as isize);
    if job.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("Codex configuration lookup could not own its process tree".into());
    }
    let job = job.unwrap();
    let mut input = child.stdin.take().ok_or("Codex stdin is missing")?;
    let output = child.stdout.take().ok_or("Codex stdout is missing")?;
    let (sender, receiver) = mpsc::sync_channel(8);
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(output);
        loop {
            let mut line = String::new();
            // Bound untrusted launcher output; never log configuration or secrets.
            if reader
                .by_ref()
                .take(4 * 1024 * 1024)
                .read_line(&mut line)
                .is_err()
                || !line.ends_with('\n')
            {
                break;
            }
            if sender.send(serde_json::from_str::<Value>(&line)).is_err() {
                break;
            }
        }
    });
    let result = (|| {
        writeln!(input, "{}", json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"miniterm_bash","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}})).map_err(|_| "Codex lookup connection closed")?;
        input
            .flush()
            .map_err(|_| "Codex lookup connection closed")?;
        let deadline = Instant::now() + Duration::from_secs(3);
        let reply = |id: u64| -> Result<Value, String> {
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let message = receiver
                    .recv_timeout(remaining)
                    .map_err(|_| "Codex configuration lookup timed out")?
                    .map_err(|_| "Codex lookup returned invalid JSON")?;
                if message.get("id").and_then(Value::as_u64) == Some(id) {
                    if message.get("error").is_some() {
                        return Err("Codex rejected configuration lookup".into());
                    }
                    return message
                        .get("result")
                        .cloned()
                        .ok_or("Codex lookup result is missing".into());
                }
            }
        };
        reply(1)?;
        writeln!(input, "{}", json!({"method":"initialized"}))
            .map_err(|_| "Codex lookup connection closed")?;
        writeln!(
            input,
            "{}",
            json!({"id":2,"method":"config/read","params":{"cwd":cwd,"includeLayers":false}})
        )
        .map_err(|_| "Codex lookup connection closed")?;
        input
            .flush()
            .map_err(|_| "Codex lookup connection closed")?;
        let mut config = reply(2)?;
        if let Some(profile) = profile.as_deref() {
            apply_profile(&mut config, profile)?;
        }
        merged_instruction(
            config
                .get("config")
                .ok_or("Codex configuration is missing")?,
        )
        .map(Some)
    })();
    drop(input);
    drop(job); // Kill the lookup server and its launcher children on every path.
    let _ = child.wait();
    drop(receiver);
    let _ = reader.join();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retains_instructions_and_literal_script_characters() {
        let original = "Keep user's instructions: 한글 ' \" $() `literal`\nsecond line";
        let merged = merged_instruction(&json!({"developer_instructions":original})).unwrap();
        let document = merged.parse::<toml_edit::DocumentMut>().unwrap();
        let value = document["developer_instructions"].as_str().unwrap();
        assert!(value.starts_with(original));
        assert!(value.contains("shell=\"bash\" and login=false"));
        assert!(!merged.contains('\n'));
        assert!(merged_instruction(&json!({})).is_err());
    }
    #[test]
    fn forwards_configuration_without_forwarding_tasks_or_permissions() {
        let args = [
            "exec",
            "--profile",
            "custom",
            "--model",
            "login",
            "-c",
            "notify=[]",
            "--sandbox",
            "read-only",
            "-a",
            "never",
            "--",
            "mcp",
        ]
        .map(String::from);
        let (flags, _, profile) = lookup_arguments(&args).unwrap();
        assert_eq!(flags, ["-c", "notify=[]"]);
        assert_eq!(profile.as_deref(), Some("custom"));
        assert!(lookup_arguments(&["exec".into(), "login".into()]).is_some());
        for values in [
            vec!["--version"],
            vec!["app-server"],
            vec!["--remote=ws://example"],
            vec!["login"],
        ] {
            assert!(
                lookup_arguments(&values.into_iter().map(String::from).collect::<Vec<_>>())
                    .is_none()
            );
        }
    }
}
