#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "notify") {
        // Hooks never block or veto an agent action. Errors contain no agent payload.
        if let Err(error) = notify(&args[1..]) {
            eprintln!("MiniTerm notify: {error}");
        }
        return Ok(());
    }
    let exe = std::env::current_exe()?;
    match args.first().map(String::as_str) {
        Some("--register") => miniterm::notification::register(&exe)?,
        Some("--unregister") => miniterm::notification::unregister(&exe)?,
        Some("--cleanup") => {
            let links = miniterm::integration::cleanup(&exe);
            let start = miniterm::native::autostart(&exe, false);
            let toast = miniterm::notification::unregister(&exe);
            links?;
            start?;
            toast?;
        }
        Some("--check-update-safe") => {
            if miniterm::ipc::running()
                || miniterm::ipc::probe(&miniterm::ipc::Request::Sessions).is_ok()
            {
                std::process::exit(3);
            }
        }
        Some("--toast-server") => miniterm::notification::activation_server()?,
        Some("toast-info") => println!("{}", miniterm::notification::diagnostics()?),
        Some("sessions") => {
            let mut reply = miniterm::ipc::send(&miniterm::ipc::Request::Sessions)?;
            reply.helper_capabilities = vec!["progress-v1".into()];
            println!("{}", serde_json::to_string(&reply)?);
        }
        Some("progress") => progress(&args[1..])?,
        Some("codex-notify-override") => {
            // Lookup is optional and silent: configuration trouble must never
            // prevent Codex from starting with its original arguments.
            if !miniterm::integration::codex_has_notification_override(&args[1..]) {
                let active = std::env::var("MINITERM_SESSION_ID").ok().filter(|id| {
                    uuid::Uuid::parse_str(id).is_ok()
                        && miniterm::ipc::send(&miniterm::ipc::Request::Sessions)
                            .is_ok_and(|reply| reply.ok && reply.sessions.contains(id))
                });
                if active.is_some() {
                    if let Ok(Some(config)) = miniterm::integration::codex_home().and_then(|home| {
                        miniterm::integration::session_codex_notify(&home, &exe, &args[1..])
                    }) {
                        println!("{config}");
                    }
                }
            }
        }
        Some("diagnostic") => {
            let reply = miniterm::ipc::send(&miniterm::ipc::Request::Diagnostic {
                command: args.get(1).ok_or("진단 명령이 필요합니다.")?.clone(),
            })?;
            if !reply.ok {
                return Err(reply.error.unwrap_or_default().into());
            }
        }
        Some("activate") => {
            let id = args.get(1).ok_or("세션 ID가 필요합니다.")?;
            let reply = miniterm::ipc::send(&miniterm::ipc::Request::Activate {
                session_id: id.clone(),
            })?;
            if !reply.ok {
                return Err(reply.error.unwrap_or_default().into());
            }
        }
        Some("integrate") => {
            let enabled = match args.get(2).map(String::as_str) {
                Some("enable") => true,
                Some("disable") => false,
                _ => return Err("integrate codex|claude enable|disable".into()),
            };
            match args.get(1).map(String::as_str) {
                Some("codex") => miniterm::integration::configure_user(
                    miniterm::agent::Agent::Codex,
                    &exe,
                    enabled,
                )?,
                Some("claude") => miniterm::integration::configure_user(
                    miniterm::agent::Agent::Claude,
                    &exe,
                    enabled,
                )?,
                _ => return Err("integrate codex|claude enable|disable".into()),
            }
        }
        _ => {
            if args.iter().any(|s| s == "--autostart")
                && miniterm::ipc::probe(&miniterm::ipc::Request::Sessions).is_ok()
            {
                return Ok(());
            }
            if miniterm::ipc::probe(&miniterm::ipc::Request::Show).is_ok() {
                return Ok(());
            }
            miniterm::notification::set_process_identity();
            miniterm::app::run().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn progress(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use miniterm::progress::{Report, Update};
    use std::io::Read;
    let session_id = std::env::var("MINITERM_SESSION_ID")
        .ok()
        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
        .ok_or("MiniTerm 터미널 안에서만 작업을 보고할 수 있습니다.")?;
    let action = args.first().map(String::as_str).unwrap_or("");
    let read_report = || -> Result<Report, Box<dyn std::error::Error>> {
        let mut raw = Vec::new();
        std::io::stdin()
            .take((miniterm::progress::MAX_BYTES + 1) as u64)
            .read_to_end(&mut raw)?;
        Ok(Report::from_bytes(&raw)?)
    };
    let receipt = || -> Result<(String, u64), Box<dyn std::error::Error>> {
        let id = args
            .get(1)
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
            .ok_or("작업 ID가 필요합니다.")?;
        let revision = args
            .get(2)
            .ok_or("다음 revision이 필요합니다.")?
            .parse::<u64>()?;
        Ok((id.clone(), revision))
    };
    let update = match (action, args.len()) {
        ("begin", 1) => Update::Begin {
            report: read_report()?,
        },
        ("update", 3) => {
            let (work_id, revision) = receipt()?;
            Update::Update {
                work_id,
                revision,
                report: read_report()?,
            }
        }
        ("clear", 3) => {
            let (work_id, revision) = receipt()?;
            Update::Clear { work_id, revision }
        }
        ("read", 1) => Update::Read,
        _ => {
            return Err(
                "progress begin|read 또는 progress update|clear <work-id> <next-revision>".into(),
            )
        }
    };
    let reply = miniterm::ipc::send(&miniterm::ipc::Request::Progress { session_id, update })?;
    if !reply.ok {
        return Err(reply.error.unwrap_or_default().into());
    }
    let output = if action == "read" {
        serde_json::to_value(reply.progress)?
    } else {
        reply
            .progress
            .map(
                |record| serde_json::json!({"workId": record.work_id, "revision": record.revision}),
            )
            .unwrap_or(serde_json::Value::Null)
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}

fn notify(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Read;
    let agent = match args.first().map(String::as_str) {
        Some("codex") => miniterm::agent::Agent::Codex,
        Some("claude") => miniterm::agent::Agent::Claude,
        _ => return Err("notify codex|claude".into()),
    };
    let raw = if agent == miniterm::agent::Agent::Codex {
        args.last()
            .ok_or("Codex JSON이 없습니다.")?
            .as_bytes()
            .to_vec()
    } else {
        let mut raw = Vec::new();
        std::io::stdin()
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut raw)?;
        raw
    };
    if agent == miniterm::agent::Agent::Codex && args.get(1).is_some_and(|s| s == "--chain-file") {
        if let (Some(path), Ok(payload)) = (args.get(2), std::str::from_utf8(&raw)) {
            let _ = miniterm::integration::chain_codex(std::path::Path::new(path), payload);
        }
    }
    // Outside a MiniTerm session, leave the existing notification callback intact.
    if let Ok(id) = std::env::var("MINITERM_SESSION_ID") {
        if let Some(event) = miniterm::agent::normalize(agent, &id, &raw)? {
            let reply = miniterm::ipc::send(&miniterm::ipc::Request::Event { event })?;
            if !reply.ok {
                return Err(reply.error.unwrap_or_default().into());
            }
        }
    }
    Ok(())
}
