//! A Git Bash executor backed by Codex's own exec-server. Conversations stay
//! on the native shared app-server; permissions and process execution remain
//! in the official executor. Only the selected shell metadata is adapted.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
};
use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Table};

const ID: &str = "miniterm-git-bash";
const MAX_FRAME: u64 = 64 * 1024 * 1024;
const MAX_INSPECT: usize = 256 * 1024;

/// Inspect small metadata only; large tool output passes through in fixed chunks.
fn relay_frames(
    reader: &mut impl BufRead,
    output: &mut impl Write,
    bash: &Path,
) -> Result<(), String> {
    let mut frame = Vec::new();
    let mut length = 0u64;
    let mut passthrough = false;
    loop {
        let bytes = reader.fill_buf().map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            return if length == 0 {
                Ok(())
            } else {
                Err("Executor protocol frame is incomplete".into())
            };
        }
        let size = bytes
            .iter()
            .position(|&b| b == b'\n')
            .map_or(bytes.len(), |n| n + 1);
        let bytes = &bytes[..size];
        length += size as u64;
        if length > MAX_FRAME {
            return Err("Executor protocol frame is too large".into());
        }
        let complete = bytes.last() == Some(&b'\n');
        if !passthrough && frame.len() + size > MAX_INSPECT {
            output.write_all(&frame).map_err(|e| e.to_string())?;
            frame.clear();
            passthrough = true;
        }
        if passthrough {
            output.write_all(bytes).map_err(|e| e.to_string())?;
        } else {
            frame.extend_from_slice(bytes);
        }
        reader.consume(size);
        if complete {
            if !passthrough {
                let mut message: Value = serde_json::from_slice(&frame)
                    .map_err(|_| "Executor protocol is incompatible")?;
                if adapt(&mut message, bash) {
                    serde_json::to_writer(&mut *output, &message).map_err(|e| e.to_string())?;
                    output.write_all(b"\n").map_err(|e| e.to_string())?;
                } else {
                    output.write_all(&frame).map_err(|e| e.to_string())?;
                }
            }
            output.flush().map_err(|e| e.to_string())?;
            frame.clear();
            length = 0;
            passthrough = false;
        }
    }
}

fn finish_executor(
    child: &mut std::process::Child,
    job: &crate::native::ProcessJob,
    clean: bool,
) -> Result<i32, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject};
    // EOF does not promise process exit. This worker must not wait indefinitely
    // for an executor which closed stdout but still has running children.
    if !clean || unsafe { WaitForSingleObject(child.as_raw_handle() as _, 200) } != WAIT_OBJECT_0 {
        job.terminate();
    }
    child
        .wait()
        .map(|s| s.code().unwrap_or(1))
        .map_err(|e| e.to_string())
}

fn adapt(message: &mut Value, bash: &Path) -> bool {
    let Some(result) = message.get_mut("result") else {
        return false;
    };
    let info = if result.get("environmentInfo").is_some() {
        &mut result["environmentInfo"]
    } else {
        result
    };
    if info.get("platformOs").and_then(Value::as_str) != Some("windows")
        || info
            .get("shell")
            .and_then(|s| s.get("name"))
            .and_then(Value::as_str)
            .is_none()
    {
        return false;
    }
    info["shell"] = serde_json::json!({"name":"bash","path":bash});
    let native = info
        .get("providerId")
        .and_then(Value::as_str)
        .unwrap_or("native");
    // This adapter changes executor behavior; do not reuse native verification IDs.
    info["providerId"] = format!("miniterm-git-bash:{}:{native}", env!("CARGO_PKG_VERSION")).into();
    true
}

pub fn serve(bash: &Path, launch: &[String]) -> Result<i32, String> {
    if !bash.is_file() {
        return Err("Git Bash 실행 파일을 찾을 수 없습니다.".into());
    }
    let mut command = Command::new(launch.first().ok_or("Codex 실행 경로가 없습니다.")?);
    command
        .args(&launch[1..])
        .args(["exec-server", "--listen", "stdio"]);
    let mut paths = vec![bash
        .parent()
        .ok_or("Bash 경로를 확인하세요.")?
        .to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    command.env(
        "PATH",
        std::env::join_paths(paths).map_err(|e| e.to_string())?,
    );
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    use std::os::windows::{io::AsRawHandle, process::CommandExt};
    command.creation_flags(0x0800_0000);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let job = match crate::native::ProcessJob::assign(child.as_raw_handle() as isize) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let mut input = child.stdin.take().ok_or("Executor stdin is missing")?;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut std::io::stdin().lock(), &mut input);
    });
    let mut reader = BufReader::new(child.stdout.take().ok_or("Executor stdout is missing")?);
    let mut output = std::io::stdout().lock();
    let result = relay_frames(&mut reader, &mut output, bash);
    let status = finish_executor(&mut child, &job, result.is_ok());
    drop(job);
    result?;
    status
}

#[derive(Serialize, Deserialize)]
struct Registration {
    original_default: Option<String>,
    helper: String,
    args: Vec<String>,
}
fn owned(table: &Table, record: &Registration) -> bool {
    table.len() == 3
        && table.get("id").and_then(|v| v.as_str()) == Some(ID)
        && table.get("program").and_then(|v| v.as_str()) == Some(record.helper.as_str())
        && table
            .get("args")
            .and_then(|v| v.as_array())
            .is_some_and(|args| {
                args.len() == record.args.len()
                    && args
                        .iter()
                        .zip(&record.args)
                        .all(|(a, b)| a.as_str() == Some(b.as_str()))
            })
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_file_name(format!(".miniterm-bash-{}.tmp", uuid::Uuid::new_v4()));
    let result = fs::write(&temp, bytes)
        .and_then(|_| fs::rename(&temp, path))
        .map_err(|e| e.to_string());
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn restore_record(path: &Path, before: Option<&[u8]>, written: &[u8]) -> Result<(), String> {
    if fs::read(path).map_err(|e| e.to_string())? != written {
        return Err("Bash 연결 기록이 변경되어 복원을 중단했습니다.".into());
    }
    match before {
        Some(bytes) => atomic(path, bytes),
        None => fs::remove_file(path).map_err(|e| e.to_string()),
    }
}
pub fn configure(
    home: &Path,
    helper: &Path,
    bash: &Path,
    launch: &[String],
    enabled: bool,
) -> Result<(), String> {
    let config = home.join("environments.toml");
    let backup = home.join("miniterm-bash.json");
    fs::create_dir_all(home).map_err(|e| e.to_string())?;
    use std::os::windows::fs::OpenOptionsExt;
    // Serialize MiniTerm writers without changing access to Codex's own files.
    let _lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(home.join("miniterm-bash.lock"))
        .map_err(|e| e.to_string())?;
    let contents = if config.exists() {
        fs::read_to_string(&config).map_err(|e| e.to_string())?
    } else {
        String::new()
    };
    let mut doc = contents.parse::<DocumentMut>().map_err(|e| e.to_string())?;
    let old_bytes = if backup.exists() {
        Some(fs::read(&backup).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let old: Option<Registration> = if let Some(bytes) = old_bytes.as_deref() {
        Some(serde_json::from_slice(bytes).map_err(|e| e.to_string())?)
    } else {
        None
    };
    if !enabled && old.is_none() {
        return Ok(());
    }
    let original_default = match doc.get("default") {
        Some(v) => Some(
            v.as_str()
                .ok_or("Codex 기본 환경 형식을 확인하세요.")?
                .to_owned(),
        ),
        None => None,
    };
    if enabled && old.is_none() && original_default.as_deref().is_some_and(|s| s != "local") {
        return Err(
            "이미 지정한 Codex 실행 환경이 있습니다. 기존 기본 환경을 먼저 확인하세요.".into(),
        );
    }
    if enabled && old.is_some() && original_default.as_deref() != Some(ID) {
        return Err("Codex의 기본 실행 환경이 변경되어 덮어쓰지 않았습니다. 연결을 해제한 뒤 다시 적용하세요.".into());
    }
    if !doc.contains_key("environments") {
        doc["environments"] = toml_edit::Item::ArrayOfTables(ArrayOfTables::new());
    }
    let entries = doc["environments"]
        .as_array_of_tables_mut()
        .ok_or("기존 Codex 환경 목록 형식을 확인하세요.")?;
    let index = entries
        .iter()
        .position(|t| t.get("id").and_then(|v| v.as_str()) == Some(ID));
    match (&old, index) {
        (Some(record), Some(i)) if owned(entries.get(i).unwrap(), record) => {}
        (None, None) => {}
        _ => return Err("MiniTerm Bash 환경 설정이 변경되어 덮어쓰지 않았습니다.".into()),
    }
    if let Some(i) = index {
        entries.remove(i);
    }
    let mut written_record = None;
    if enabled {
        if !bash.is_file() || launch.is_empty() || !Path::new(&launch[0]).is_file() {
            return Err("Bash와 Codex 실행 경로를 확인하세요.".into());
        }
        let mut args = vec![
            "codex-bash-server".into(),
            bash.to_string_lossy().into_owned(),
            "--".into(),
        ];
        args.extend_from_slice(launch);
        let record = Registration {
            original_default: old.map(|r| r.original_default).unwrap_or(original_default),
            helper: helper.to_string_lossy().into_owned(),
            args,
        };
        let mut table = Table::new();
        table["id"] = value(ID);
        table["program"] = value(&record.helper);
        let mut array = Array::new();
        for arg in &record.args {
            array.push(arg.as_str());
        }
        table["args"] = value(array);
        entries.push(table);
        doc["default"] = value(ID);
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        atomic(&backup, &bytes)?;
        written_record = Some(bytes);
    } else if doc.get("default").and_then(|v| v.as_str()) == Some(ID) {
        if let Some(original) = old.unwrap().original_default {
            doc["default"] = value(original);
        } else {
            doc.remove("default");
        }
    }
    let updated = doc.to_string();
    if let Err(error) = atomic(&config, updated.as_bytes()) {
        if let Some(bytes) = written_record {
            restore_record(&backup, old_bytes.as_deref(), &bytes)
                .map_err(|rollback| format!("{error}; {rollback}"))?;
        }
        return Err(error);
    }
    if !enabled {
        if let Err(error) = fs::remove_file(&backup) {
            // Keep an unsuccessful disconnect retryable. Never undo an edit
            // made by another writer after this function's config replacement.
            if fs::read_to_string(&config).ok().as_deref() == Some(&updated) {
                atomic(&config, contents.as_bytes())?;
            }
            return Err(error.to_string());
        }
    }
    Ok(())
}

pub fn configure_user(helper: &Path, enabled: bool) -> Result<(), String> {
    configure_user_with_bash(helper, None, enabled)
}

pub fn configure_user_with_bash(
    helper: &Path,
    selected: Option<&Path>,
    enabled: bool,
) -> Result<(), String> {
    let home = crate::integration::codex_home()?;
    if !enabled {
        return configure(&home, helper, Path::new(""), &[], false);
    }
    let bash = selected
        .map(Path::to_path_buf)
        .or_else(crate::shell::detect_git_bash)
        .ok_or("설치된 Git Bash를 찾을 수 없습니다.")?;
    let launch =
        crate::shell::codex_executor_launch().ok_or("Codex 실행 파일을 찾을 수 없습니다.")?;
    configure(&home, helper, &bash, &launch, enabled)
}

pub fn cleanup_user(helper: &Path) -> Result<(), String> {
    let home = crate::integration::codex_home()?;
    let backup = home.join("miniterm-bash.json");
    if backup.is_file() {
        let record: Registration =
            serde_json::from_slice(&fs::read(backup).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if Path::new(&record.helper) == helper {
            configure(&home, helper, Path::new(""), &[], false)?;
        }
    }
    Ok(())
}

fn registered_launch(home: &Path) -> Result<Option<Vec<String>>, String> {
    let record_path = home.join("miniterm-bash.json");
    if !record_path.is_file() {
        return Ok(None);
    }
    let record: Registration =
        serde_json::from_slice(&fs::read(record_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let document: DocumentMut = fs::read_to_string(home.join("environments.toml"))
        .map_err(|e| e.to_string())?
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    if document.get("default").and_then(|v| v.as_str()) != Some(ID) {
        return Ok(None);
    }
    let entry = document
        .get("environments")
        .and_then(|v| v.as_array_of_tables())
        .and_then(|v| {
            v.iter()
                .find(|v| v.get("id").and_then(|v| v.as_str()) == Some(ID))
        });
    if !entry.is_some_and(|v| owned(v, &record))
        || record.args.get(2).map(String::as_str) != Some("--")
        || record.args.len() < 4
    {
        return Err("Bash 실행 환경 등록을 확인하세요.".into());
    }
    Ok(Some(record.args[3..].to_vec()))
}

fn shared_ui_call(args: &[String]) -> bool {
    !args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "--no-daemon"
                | "--remote"
                | "--strict-config"
                | "--profile"
                | "-p"
                | "-c"
                | "--config"
                | "--help"
                | "-h"
                | "--version"
                | "-V"
                | "exec"
                | "e"
                | "review"
                | "app-server"
                | "exec-server"
                | "mcp"
                | "mcp-server"
                | "login"
                | "logout"
                | "completion"
                | "debug"
                | "apply"
                | "sandbox"
        ) || arg.starts_with("--remote=")
            || arg.starts_with("--profile=")
            || arg.starts_with("--config=")
            || (arg.starts_with("-c") && !arg.starts_with("--"))
    })
}

struct Preparation {
    session: String,
    started: std::time::Instant,
    result: Option<Result<Option<String>, String>>,
}
static PREPARATIONS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::BTreeMap<String, Preparation>>,
> = std::sync::LazyLock::new(Default::default);

/// Called by the GUI's IPC worker, outside every terminal ProcessJob. The
/// managed server must outlive its frontend and must start before console input.
pub fn begin_preparation(
    session: &str,
    home: &Path,
    cwd: &Path,
    environment: std::collections::BTreeMap<String, String>,
) -> crate::ipc::Reply {
    let launch = match registered_launch(home) {
        Ok(Some(launch)) => launch,
        Ok(None) => match crate::shell::codex_executor_launch() {
            Some(launch) => launch,
            None => return crate::ipc::Reply::error("Codex 실행 파일을 찾을 수 없습니다."),
        },
        Err(error) => return crate::ipc::Reply::error(error),
    };
    if !home.is_absolute() || !cwd.is_dir() {
        return crate::ipc::Reply::error("Codex 실행 경로를 확인하세요.");
    }
    let ticket = uuid::Uuid::new_v4().to_string();
    {
        let mut jobs = PREPARATIONS.lock().unwrap_or_else(|p| p.into_inner());
        jobs.retain(|_, v| v.started.elapsed().as_secs() < 60);
        if jobs.len() >= 8 {
            return crate::ipc::Reply::error("진행 중인 Codex 연결 준비를 기다려 주세요.");
        }
        jobs.insert(
            ticket.clone(),
            Preparation {
                session: session.into(),
                started: std::time::Instant::now(),
                result: None,
            },
        );
    }
    let worker_ticket = ticket.clone();
    let home = home.to_owned();
    let cwd = cwd.to_owned();
    let session = session.to_owned();
    std::thread::spawn(move || {
        use std::os::windows::process::CommandExt;
        let result = (|| {
            let mut command = Command::new(&launch[0]);
            command
                .args(&launch[1..])
                .args(["app-server", "daemon", "start"])
                .current_dir(&cwd)
                // Preserve the launching shell's API/tool environment rather
                // than using values captured when the GUI first started.
                .env_clear()
                .envs(&environment)
                .env("CODEX_HOME", &home)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .creation_flags(0x0800_0000);
            // A managed server has its own lifetime, not the caller's thread.
            for name in [
                "CODEX_DAEMON_SHUTDOWN_SOCKET",
                "CODEX_SESSION_ID",
                "CODEX_THREAD_ID",
                "MINITERM_SESSION_ID",
                "MINITERM_INSTANCE_ID",
                "MINITERM_NOTIFY_EXE",
                "MINITERM_CODEX_LINK_TOKEN",
            ] {
                command.env_remove(name);
            }
            let mut child = command.spawn().map_err(|e| e.to_string())?;
            let errors =
                match crate::command_output::ErrorOutput::capture(child.stderr.take().unwrap()) {
                    Ok(errors) => errors,
                    Err(error) => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(error.to_string());
                    }
                };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        return if status.success() {
                            if crate::integration::codex_link_enabled(&home) {
                                crate::codex_link::start(
                                    &session,
                                    &launch,
                                    &home,
                                    &cwd,
                                    &environment,
                                )
                                .map(Some)
                            } else {
                                Ok(None)
                            }
                        } else {
                            let detail = errors.message();
                            Err(format!(
                                "Codex 공유 서버를 준비하지 못했습니다 ({status}). {detail}"
                            ))
                        }
                    }
                    Ok(None) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(std::time::Duration::from_millis(50))
                    }
                    _ => {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("Codex 공유 서버 준비 시간이 초과됐습니다.".into());
                    }
                }
            }
        })();
        if let Some(job) = PREPARATIONS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(&worker_ticket)
        {
            job.result = Some(result);
        }
    });
    crate::ipc::Reply {
        preparation_id: Some(ticket),
        ..Default::default()
    }
}

pub fn poll_preparation(session: &str, ticket: &str) -> crate::ipc::Reply {
    let mut jobs = PREPARATIONS.lock().unwrap_or_else(|p| p.into_inner());
    let Some(job) = jobs.get(ticket) else {
        return crate::ipc::Reply::error("Codex 연결 준비를 다시 시작하세요.");
    };
    if job.session != session {
        return crate::ipc::Reply::error("Codex를 실행한 탭을 확인하세요.");
    }
    let Some(result) = job.result.clone() else {
        return crate::ipc::Reply {
            preparation_id: Some(ticket.into()),
            ..Default::default()
        };
    };
    jobs.remove(ticket);
    match result {
        Ok(codex_connection) => crate::ipc::Reply {
            codex_connection,
            ..crate::ipc::Reply::ok()
        },
        Err(error) => crate::ipc::Reply::error(error),
    }
}

pub fn prepare_foreground(args: &[String]) -> Result<(), String> {
    if !shared_ui_call(args) {
        return Ok(());
    }
    let home = crate::integration::codex_home()?;
    let Ok(session) = std::env::var("MINITERM_SESSION_ID") else {
        return Ok(());
    };
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let home = if home.is_absolute() {
        home
    } else {
        cwd.join(home)
    };
    let environment = std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    let mut reply = crate::ipc::send(&crate::ipc::Request::PrepareCodex {
        session_id: session.clone(),
        home,
        cwd,
        environment,
    })
    .map_err(|e| e.to_string())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    while let Some(ticket) = reply.preparation_id {
        if std::time::Instant::now() >= deadline {
            return Err("Codex 연결 준비 시간이 초과됐습니다.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(75));
        reply = crate::ipc::send(&crate::ipc::Request::CodexPrepared {
            session_id: session.clone(),
            ticket,
        })
        .map_err(|e| e.to_string())?;
    }
    if reply.ok {
        if let Some(endpoint) = reply.codex_connection {
            println!("{endpoint}");
        }
        Ok(())
    } else {
        Err(reply
            .error
            .unwrap_or_else(|| "Codex 공유 서버를 준비하지 못했습니다.".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_executor_frames_stream_unchanged_then_metadata_is_adapted() {
        use std::io::{BufReader, Cursor};
        let mut frame = b"{\"result\":{\"output\":\"".to_vec();
        frame.extend(std::iter::repeat_n(b'x', MAX_INSPECT * 5));
        frame.extend_from_slice(b"\"}}\n");
        let metadata = b"{\"id\":1,\"result\":{\"platformOs\":\"windows\",\"shell\":{\"name\":\"powershell\"}}}\n";
        let input = [frame.as_slice(), metadata].concat();
        let mut output = Vec::new();
        relay_frames(
            &mut BufReader::with_capacity(117, Cursor::new(input)),
            &mut output,
            Path::new("C:/Git/bin/bash.exe"),
        )
        .unwrap();
        assert_eq!(&output[..frame.len()], &frame);
        let adapted: Value = serde_json::from_slice(&output[frame.len()..]).unwrap();
        assert_eq!(adapted["result"]["shell"]["name"], "bash");
    }
    #[test]
    fn rejects_unterminated_and_oversized_executor_frames_without_large_buffering() {
        use std::io::{BufReader, Cursor, Read};
        let bash = Path::new("C:/Git/bin/bash.exe");
        assert!(relay_frames(&mut Cursor::new(b"{\"id\":1}"), &mut std::io::sink(), bash).is_err());
        let input = std::io::repeat(b'x').take(MAX_FRAME + 1);
        let mut input = BufReader::with_capacity(8192, input);
        assert!(relay_frames(&mut input, &mut std::io::sink(), bash)
            .unwrap_err()
            .contains("too large"));
    }
    #[test]
    fn executor_eof_cleanup_terminates_an_owned_process_that_does_not_exit() {
        use std::{
            os::windows::{io::AsRawHandle, process::CommandExt},
            time::{Duration, Instant},
        };
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(0x0800_0000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let job = crate::native::ProcessJob::assign(child.as_raw_handle() as isize).unwrap();
        let start = Instant::now();
        let code = finish_executor(&mut child, &job, true).unwrap();
        assert_ne!(code, 0);
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn preparation_keeps_explicit_native_backend_and_cli_commands() {
        for args in [
            vec![],
            vec!["--no-alt-screen"],
            vec!["resume", "--last"],
            vec!["fork"],
        ] {
            assert!(shared_ui_call(
                &args.into_iter().map(str::to_owned).collect::<Vec<_>>()
            ));
        }
        for args in [
            vec!["--no-daemon"],
            vec!["-c", "notify=[]"],
            vec!["--config=notify=[]"],
            vec!["--profile", "custom"],
            vec!["--remote=unix://test"],
            vec!["--strict-config"],
            vec!["exec", "--json"],
            vec!["app-server", "daemon", "status"],
            vec!["--version"],
        ] {
            assert!(!shared_ui_call(
                &args.into_iter().map(str::to_owned).collect::<Vec<_>>()
            ));
        }
    }
    #[test]
    fn changes_only_executor_shell_and_verification_identity() {
        let mut message = serde_json::json!({"id":1,"result":{"sessionId":"original","environmentInfo":{"platformOs":"windows","shell":{"name":"powershell","path":"pwsh.exe"},"providerId":"native","cwd":"original","capabilities":{"shellSnapshotV2":false}}}});
        assert!(adapt(&mut message, Path::new("C:/Git/bash.exe")));
        let info = &message["result"]["environmentInfo"];
        assert_eq!(info["shell"]["name"], "bash");
        assert_eq!(info["cwd"], "original");
        assert_eq!(info["capabilities"]["shellSnapshotV2"], false);
        assert_ne!(info["providerId"], "native");
        let mut request = serde_json::json!({"method":"process/create","params":{"shell":{"name":"powershell"},"sandboxPolicy":{"type":"readOnly"}}});
        let original = request.clone();
        assert!(!adapt(&mut request, Path::new("bash.exe")));
        assert_eq!(request, original);
    }
    #[test]
    fn registration_preserves_other_environments_and_user_changes() {
        let home = std::env::temp_dir().join(format!("miniterm-bash-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let exe = std::env::current_exe().unwrap();
        let launch = vec![exe.to_string_lossy().into_owned()];
        let original="default='local'\ninclude_local=true\n# user entry\n[[environments]]\nid='other'\nurl='ws://127.0.0.1:12345'\n";
        fs::write(home.join("environments.toml"), original).unwrap();
        configure(&home, &exe, &exe, &launch, true).unwrap();
        configure(&home, &exe, &exe, &launch, true).unwrap();
        let installed = fs::read_to_string(home.join("environments.toml")).unwrap();
        assert!(installed.contains("# user entry"));
        assert_eq!(installed.matches("miniterm-git-bash").count(), 2);
        let changed = installed.replace("default = \"miniterm-git-bash\"", "default = \"other\"");
        fs::write(home.join("environments.toml"), changed).unwrap();
        configure(&home, &exe, &exe, &launch, false).unwrap();
        let restored = fs::read_to_string(home.join("environments.toml")).unwrap();
        assert!(!restored.contains(ID));
        assert!(restored.contains("other"));
        fs::remove_file(home.join("environments.toml")).unwrap();
        fs::remove_file(home.join("miniterm-bash.lock")).unwrap();
        fs::remove_dir(home).unwrap();
    }
    #[test]
    fn config_write_failure_restores_registration_and_allows_retry() {
        let home = std::env::temp_dir().join(format!("miniterm-bash-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&home).unwrap();
        let exe = std::env::current_exe().unwrap();
        let launch = vec![exe.to_string_lossy().into_owned()];
        let config = home.join("environments.toml");
        fs::write(&config, "default='local'\n").unwrap();
        let original_permissions = fs::metadata(&config).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&config, permissions.clone()).unwrap();
        assert!(configure(&home, &exe, &exe, &launch, true).is_err());
        assert!(!home.join("miniterm-bash.json").exists());
        assert_eq!(fs::read_to_string(&config).unwrap(), "default='local'\n");
        fs::set_permissions(&config, original_permissions).unwrap();
        configure(&home, &exe, &exe, &launch, true).unwrap();
        configure(&home, &exe, &exe, &launch, false).unwrap();
        assert!(!home.join("miniterm-bash.json").exists());
        fs::remove_file(config).unwrap();
        fs::remove_file(home.join("miniterm-bash.lock")).unwrap();
        fs::remove_dir(home).unwrap();
    }
}
