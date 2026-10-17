//! Observe a single TUI connection to the existing managed Codex server.
//! The official proxy owns the WebSocket handshake and protocol. Bytes are
//! forwarded unchanged; only successful frontend thread bindings and lifecycle
//! metadata are retained. No prompts, responses, or credentials are logged.
use crate::agent::{Agent, AgentEvent, State};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const MAX_METADATA: usize = 4 * 1024 * 1024;
// The TUI opens auxiliary connections for /new and the session picker while
// retaining its foreground connection. Bound resources without serializing them.
const MAX_CONNECTIONS: usize = 4;

struct Connection {
    stream: TcpStream,
    job: Mutex<Option<Arc<crate::native::ProcessJob>>>,
}
impl Connection {
    fn close(&self) {
        let _ = self.stream.shutdown(Shutdown::Both);
        if let Some(job) = self.job.lock().unwrap_or_else(|p| p.into_inner()).take() {
            job.terminate();
        }
    }
}
type Connections = Arc<Mutex<HashMap<u64, Arc<Connection>>>>;

struct ConnectionGuard {
    id: u64,
    connections: Connections,
    connection: Arc<Connection>,
}
impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.connection.close();
        self.connections
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.id);
    }
}

#[derive(Default)]
struct Observer {
    pending: HashMap<String, String>,
    thread: Option<String>,
    cwd: String,
    last: Option<(String, String, State)>,
    sequence: u64,
}
impl Observer {
    fn request(&mut self, value: &Value) {
        let Some(method) = value["method"].as_str() else {
            return;
        };
        if !matches!(
            method,
            "thread/start" | "thread/resume" | "thread/fork" | "thread/unsubscribe"
        ) || value["params"]["ephemeral"] == true
        {
            return;
        }
        let Some(id) = value.get("id").filter(|v| v.is_string() || v.is_number()) else {
            return;
        };
        let id = id.to_string();
        if id.len() > 256 || self.pending.len() >= 32 {
            return;
        }
        let action = if method == "thread/unsubscribe" {
            format!(
                "unsubscribe:{}",
                value["params"]["threadId"].as_str().unwrap_or_default()
            )
        } else {
            "bind".into()
        };
        self.pending.insert(id, action);
    }

    fn response(&mut self, value: &Value, session: &str) -> Option<AgentEvent> {
        if let Some(id) = value.get("id").filter(|_| {
            value.get("method").is_none()
                && (value.get("result").is_some() || value.get("error").is_some())
        }) {
            if let Some(action) = self.pending.remove(&id.to_string()) {
                // An unsuccessful resume must never steal another tab's events.
                if value.get("error").is_some() {
                    return None;
                }
                // The TUI also starts ephemeral helper conversations (for
                // example, generating a title). They share the transport but
                // must never replace the user's foreground conversation.
                if value["result"]["thread"]["ephemeral"] == true
                    || value["result"]["thread"]["source"]
                        .get("subAgent")
                        .is_some()
                {
                    return None;
                }
                if let Some(id) = action.strip_prefix("unsubscribe:") {
                    if self.thread.as_deref() == Some(id) {
                        self.thread = None;
                        self.last = None;
                    }
                } else if let Some(id) = value["result"]["thread"]["id"]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 128)
                {
                    self.thread = Some(id.into());
                    self.cwd = value["result"]["thread"]["cwd"]
                        .as_str()
                        .unwrap_or_default()
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(2048)
                        .collect();
                    self.last = None;
                }
                return None;
            }
        }
        let params = &value["params"];
        let id = params["threadId"].as_str()?;
        if self.thread.as_deref() != Some(id) {
            return None;
        }
        let method = value["method"].as_str()?;
        let previous_turn = self.last.as_ref().map(|(_, turn, _)| turn.clone());
        let (state, turn) = match method {
            "turn/started" => (State::Running, params["turn"]["id"].as_str()?),
            "turn/completed" => {
                let state = match params["turn"]["status"].as_str()? {
                    "completed" => State::Complete,
                    "failed" => State::Failed,
                    "interrupted" => State::Interrupted,
                    _ => return None,
                };
                (state, params["turn"]["id"].as_str()?)
            }
            "item/commandExecution/requestApproval"
            | "item/fileChange/requestApproval"
            | "item/tool/requestUserInput" => (State::NeedsInput, params["turnId"].as_str()?),
            "thread/status/changed" if params["status"]["type"] == "active" => {
                // Thread activity can outlive a completed turn (for example,
                // background bookkeeping). Only turn/started begins new work.
                // Late thread status must not clear the completion badge.
                if !matches!(self.last.as_ref()?.2, State::Running | State::NeedsInput) {
                    return None;
                }
                let flags = params["status"]["activeFlags"].as_array()?;
                let waiting = flags.iter().any(|flag| {
                    matches!(
                        flag.as_str(),
                        Some("waitingOnApproval" | "waitingOnUserInput")
                    )
                });
                (
                    if waiting {
                        State::NeedsInput
                    } else {
                        State::Running
                    },
                    previous_turn.as_deref()?,
                )
            }
            _ => return None,
        };
        if turn.is_empty() || turn.len() > 128 {
            return None;
        }
        let key = (id.to_owned(), turn.to_owned(), state);
        if self.last.as_ref() == Some(&key) {
            return None;
        }
        self.last = Some(key);
        self.sequence = self.sequence.wrapping_add(1);
        let event = AgentEvent {
            session_id: session.into(),
            agent: Agent::Codex,
            agent_session_id: id.into(),
            // Match the existing official notify key for completed turns.
            event_id: Some(if state == State::Complete {
                turn.into()
            } else {
                format!("{turn}:{}", self.sequence)
            }),
            state,
            cwd: self.cwd.clone(),
            occurred_at_ms: crate::agent::now_ms(),
        };
        event.validate().ok()?;
        Some(event)
    }
}

/// Passive RFC 6455 text-message observer. Unsupported or oversized frames are
/// skipped without interrupting their original byte stream. Bounds apply to
/// metadata inspection, not the transport (which can carry large images).
#[derive(Default)]
struct Frames {
    header: Vec<u8>,
    remaining: u64,
    offset: u64,
    mask: Option<[u8; 4]>,
    opcode: u8,
    fin: bool,
    message: Vec<u8>,
    text: bool,
    overflow: bool,
}
impl Frames {
    fn feed(&mut self, bytes: &[u8], mut message: impl FnMut(Value)) {
        for &byte in bytes {
            if self.remaining > 0 {
                if self.text && !self.overflow && self.opcode < 8 {
                    if self.message.len() == MAX_METADATA {
                        self.message.clear();
                        self.overflow = true;
                    } else {
                        self.message
                            .push(byte ^ self.mask.map_or(0, |m| m[self.offset as usize % 4]));
                    }
                }
                self.offset += 1;
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.finish(&mut message);
                }
                continue;
            }
            self.header.push(byte);
            if self.header.len() < 2 {
                continue;
            }
            let extra = match self.header[1] & 127 {
                126 => 2,
                127 => 8,
                _ => 0,
            };
            let masked = self.header[1] & 128 != 0;
            let size = 2 + extra + if masked { 4 } else { 0 };
            if self.header.len() < size {
                continue;
            }
            self.opcode = self.header[0] & 15;
            self.fin = self.header[0] & 128 != 0;
            self.remaining = if extra == 0 {
                (self.header[1] & 127) as u64
            } else {
                self.header[2..2 + extra]
                    .iter()
                    .fold(0u64, |n, &b| (n << 8) | u64::from(b))
            };
            self.mask = masked.then(|| self.header[2 + extra..size].try_into().unwrap());
            self.offset = 0;
            if self.opcode == 1 || self.opcode == 2 {
                self.message.clear();
                self.text = self.opcode == 1;
                self.overflow = false;
            }
            if self.header[0] & 112 != 0 {
                self.overflow = true;
            }
            self.header.clear();
            if self.remaining == 0 {
                self.finish(&mut message);
            }
        }
    }
    fn finish(&mut self, message: &mut impl FnMut(Value)) {
        if self.fin && self.opcode < 8 {
            if self.text && !self.overflow {
                if let Ok(value) = serde_json::from_slice(&self.message) {
                    message(value);
                }
            }
            self.message.clear();
            self.text = false;
            self.overflow = false;
        }
    }
}

struct Link {
    session: String,
    stop: Arc<AtomicBool>,
    connections: Connections,
    finished: Arc<AtomicBool>,
}
impl Drop for Link {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for connection in self
            .connections
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
        {
            connection.close();
        }
    }
}
static LINKS: LazyLock<Mutex<HashMap<String, Link>>> = LazyLock::new(Default::default);

pub fn active(session: &str, connection: &str) -> bool {
    LINKS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(connection)
        .is_some_and(|link| {
            link.session == session
                && !link.stop.load(Ordering::Acquire)
                && !link.finished.load(Ordering::Acquire)
        })
}

pub fn close(session: &str, endpoint: &str) {
    let mut links = LINKS.lock().unwrap_or_else(|p| p.into_inner());
    if links
        .get(endpoint)
        .is_some_and(|link| link.session == session)
    {
        links.remove(endpoint);
    }
}
pub fn close_session(session: &str) {
    LINKS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|_, link| link.session != session);
}
pub fn close_all() {
    LINKS.lock().unwrap_or_else(|p| p.into_inner()).clear();
}

pub fn start(
    session: &str,
    launch: &[String],
    home: &Path,
    cwd: &Path,
    environment: &BTreeMap<String, String>,
) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let endpoint = format!(
        "ws://127.0.0.1:{} {token}",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    let stop = Arc::new(AtomicBool::new(false));
    let connections = Connections::default();
    let finished = Arc::new(AtomicBool::new(false));
    let mut links = LINKS.lock().unwrap_or_else(|p| p.into_inner());
    links.retain(|_, link| !link.finished.load(Ordering::Acquire));
    if links.len() >= 8 {
        return Err("진행 중인 Codex 알림 연결을 확인하세요.".into());
    }
    links.insert(
        endpoint.clone(),
        Link {
            session: session.into(),
            stop: stop.clone(),
            connections: connections.clone(),
            finished: finished.clone(),
        },
    );
    let launch = launch.to_vec();
    let home = home.to_owned();
    let cwd = cwd.to_owned();
    let environment = environment.clone();
    let session = session.to_owned();
    let connection = endpoint.clone();
    let worker = thread::Builder::new()
        .name("codex-listener".into())
        .spawn(move || {
            let mut deadline = Instant::now() + Duration::from_secs(30);
            let mut next_id = 0u64;
            while !stop.load(Ordering::Acquire) && Instant::now() < deadline {
                if !connections
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .is_empty()
                {
                    deadline = Instant::now() + Duration::from_secs(30);
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let Ok(active) = stream.try_clone() else {
                            continue;
                        };
                        let active = Arc::new(Connection {
                            stream: active,
                            job: Mutex::new(None),
                        });
                        let mut slots = connections.lock().unwrap_or_else(|p| p.into_inner());
                        if slots.len() >= MAX_CONNECTIONS {
                            // Drop only this excess connection. Existing TUI work remains live.
                            continue;
                        }
                        let id = next_id;
                        next_id = next_id.wrapping_add(1);
                        slots.insert(id, active.clone());
                        drop(slots);
                        let guard = ConnectionGuard {
                            id,
                            connections: connections.clone(),
                            connection: active,
                        };
                        let (token, session, launch, home, cwd, environment, stop, connection) = (
                            token.clone(),
                            session.clone(),
                            launch.clone(),
                            home.clone(),
                            cwd.clone(),
                            environment.clone(),
                            stop.clone(),
                            connection.clone(),
                        );
                        // A failed spawn drops the closure and its guard, releasing the slot.
                        let _ =
                            thread::Builder::new()
                                .name("codex-relay".into())
                                .spawn(move || {
                                    let guard = guard;
                                    let _ = relay(
                                        stream,
                                        &token,
                                        &session,
                                        &launch,
                                        &home,
                                        &cwd,
                                        &environment,
                                        &stop,
                                        &guard.connection.job,
                                        &connection,
                                    );
                                });
                        deadline = Instant::now() + Duration::from_secs(30);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50))
                    }
                    Err(_) => break,
                }
            }
            finished.store(true, Ordering::Release);
        });
    if let Err(error) = worker {
        links.remove(&endpoint);
        return Err(error.to_string());
    }
    Ok(endpoint)
}

// Keep socket, process and connection ownership explicit at the relay boundary.
#[allow(clippy::too_many_arguments)]
fn relay(
    mut stream: TcpStream,
    token: &str,
    session: &str,
    launch: &[String],
    home: &Path,
    cwd: &Path,
    environment: &BTreeMap<String, String>,
    stop: &AtomicBool,
    job_slot: &Mutex<Option<Arc<crate::native::ProcessJob>>>,
    connection: &str,
) -> std::io::Result<()> {
    // Winsock accept inherits the nonblocking listener's socket mode. Timeouts
    // do not restore blocking reads: without this, idle WouldBlock retries spin
    // on one CPU core for every active TUI connection.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_millis(200)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    // Codex accepts a root WebSocket URL and a bearer-token environment variable.
    // Validate that token before opening the user's daemon.
    let mut header = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !header.ends_with(b"\r\n\r\n") {
        if stop.load(Ordering::Acquire) || Instant::now() >= deadline || header.len() >= 8192 {
            return Ok(());
        }
        let mut byte = [0];
        match stream.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => header.push(byte[0]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                continue
            }
            Err(e) => return Err(e),
        }
    }
    let first_end = header
        .windows(2)
        .position(|b| b == b"\r\n")
        .unwrap_or(header.len());
    let authorization = format!("Bearer {token}");
    let headers = String::from_utf8_lossy(&header);
    let auth: Vec<_> = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.trim())
        .collect();
    if header[..first_end] != *b"GET / HTTP/1.1" || auth != [authorization.as_str()] {
        stream.write_all(
            b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(());
    }
    let mut child = Command::new(&launch[0])
        .args(&launch[1..])
        .args(["app-server", "proxy"])
        .current_dir(cwd)
        .env_clear()
        .envs(environment)
        .env("CODEX_HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000)
        .spawn()?;
    let job = match crate::native::ProcessJob::assign(child.as_raw_handle() as isize) {
        Ok(job) => Arc::new(job),
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::other(error));
        }
    };
    *job_slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(job.clone());
    if stop.load(Ordering::Acquire) {
        job.terminate();
        let _ = child.wait();
        return Ok(());
    }
    let result = (|| {
        let mut input = child.stdin.take().unwrap();
        let mut output = child.stdout.take().unwrap();
        input.write_all(b"GET / HTTP/1.1")?;
        input.write_all(&header[first_end..])?;
        let mut outgoing = stream.try_clone()?;
        let observer = Arc::new(Mutex::new(Observer::default()));
        let incoming_observer = observer.clone();
        let session = session.to_owned();
        let connection = connection.to_owned();
        let reporter = crate::event_reporter::Reporter::start(move |event| {
            let _ = crate::ipc::send(&crate::ipc::Request::CodexEvent {
                connection: connection.clone(),
                event,
            });
        })?;
        let reader = thread::Builder::new()
            .name("codex-output".into())
            .spawn(move || {
                let mut frames = Frames::default();
                let mut header = Vec::new();
                let mut ready = false;
                let mut bytes = [0; 16 * 1024];
                while let Ok(count) = output.read(&mut bytes) {
                    if count == 0 || outgoing.write_all(&bytes[..count]).is_err() {
                        break;
                    }
                    let mut start = 0;
                    if !ready {
                        for (i, &b) in bytes[..count].iter().enumerate() {
                            header.push(b);
                            if header.ends_with(b"\r\n\r\n") {
                                ready = header.starts_with(b"HTTP/1.1 101 ");
                                start = i + 1;
                                break;
                            }
                            if header.len() > 8192 {
                                break;
                            }
                        }
                        if !ready {
                            if header.len() > 8192 || header.ends_with(b"\r\n\r\n") {
                                break;
                            }
                            continue;
                        }
                        header.clear();
                    }
                    frames.feed(&bytes[start..count], |value| {
                        let event = incoming_observer
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .response(&value, &session);
                        if let Some(event) = event {
                            reporter.report(event);
                        }
                    });
                }
                let _ = outgoing.shutdown(Shutdown::Both);
            })?;
        let mut frames = Frames::default();
        let mut bytes = [0; 16 * 1024];
        while !stop.load(Ordering::Acquire) {
            match stream.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    // Observe before forwarding: even a fast response must find its request.
                    frames.feed(&bytes[..count], |value| {
                        observer
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .request(&value)
                    });
                    if input.write_all(&bytes[..count]).is_err() {
                        break;
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    continue
                }
                Err(_) => break,
            }
        }
        drop(input);
        job.terminate();
        let _ = stream.shutdown(Shutdown::Both);
        let _ = reader.join();
        Ok(())
    })();
    job.terminate();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const SID: &str = "71a461b4-1259-4ac7-b321-243d742c54b4";
    #[test]
    fn request_id_namespaces_and_approval_resumption_are_independent() {
        let mut observer = Observer::default();
        observer.request(&json!({"id":1,"method":"thread/resume"}));
        assert!(observer.response(&json!({"id":1,"method":"item/tool/requestUserInput","params":{"threadId":"a","turnId":"t"}}),SID).is_none());
        observer.response(&json!({"id":1,"result":{"thread":{"id":"a"}}}), SID);
        assert_eq!(
            observer
                .response(
                    &json!({"method":"turn/started","params":{"threadId":"a","turn":{"id":"t"}}}),
                    SID
                )
                .unwrap()
                .state,
            State::Running
        );
        assert_eq!(observer.response(&json!({"id":1,"method":"item/commandExecution/requestApproval","params":{"threadId":"a","turnId":"t"}}),SID).unwrap().state,State::NeedsInput);
        assert_eq!(observer.response(&json!({"method":"thread/status/changed","params":{"threadId":"a","status":{"type":"active","activeFlags":[]}}}),SID).unwrap().state,State::Running);
    }
    #[test]
    fn late_thread_activity_keeps_completion_until_the_next_turn_starts() {
        let mut observer = Observer::default();
        observer.request(&json!({"id":1,"method":"thread/start"}));
        observer.response(&json!({"id":1,"result":{"thread":{"id":"a"}}}), SID);
        for status in ["completed", "failed", "interrupted"] {
            observer.response(&json!({"method":"turn/completed","params":{"threadId":"a","turn":{"id":"t","status":status}}}), SID).unwrap();
            for flags in [json!([]), json!(["waitingOnApproval"])] {
                assert!(observer.response(&json!({"method":"thread/status/changed","params":{"threadId":"a","status":{"type":"active","activeFlags":flags}}}), SID).is_none());
            }
        }
        assert_eq!(observer.response(&json!({"method":"turn/started","params":{"threadId":"a","turn":{"id":"next"}}}), SID).unwrap().state, State::Running);
        assert_eq!(observer.response(&json!({"method":"thread/status/changed","params":{"threadId":"a","status":{"type":"active","activeFlags":["waitingOnUserInput"]}}}), SID).unwrap().state, State::NeedsInput);
    }

    #[test]
    fn helper_conversations_cannot_replace_the_frontend_binding() {
        let mut observer = Observer::default();
        observer.request(&json!({"id":1,"method":"thread/start"}));
        observer.response(&json!({"id":1,"result":{"thread":{"id":"main"}}}), SID);
        observer.request(&json!({"id":2,"method":"thread/start","params":{"ephemeral":true}}));
        observer.response(
            &json!({"id":2,"result":{"thread":{"id":"title","ephemeral":true}}}),
            SID,
        );
        observer.request(&json!({"id":3,"method":"thread/resume"}));
        observer.response(
            &json!({"id":3,"result":{"thread":{"id":"helper","ephemeral":true}}}),
            SID,
        );
        observer.request(&json!({"id":4,"method":"thread/start"}));
        observer.response(
            &json!({"id":4,"result":{"thread":{"id":"child","source":{"subAgent":"review"}}}}),
            SID,
        );
        for id in ["title", "helper", "child", "main"] {
            let event = observer.response(&json!({"method":"turn/completed","params":{"threadId":id,"turn":{"id":"t","status":"completed"}}}), SID);
            assert_eq!(event.is_some(), id == "main");
        }
    }
    #[test]
    fn idle_relay_waits_on_the_socket_instead_of_spinning() {
        use windows_sys::Win32::{
            Foundation::FILETIME,
            System::Threading::{GetCurrentThread, GetThreadTimes},
        };
        fn cpu_time() -> u64 {
            let mut times: [FILETIME; 4] = unsafe { std::mem::zeroed() };
            let [created, exited, kernel, user] = &mut times;
            assert_ne!(
                unsafe { GetThreadTimes(GetCurrentThread(), created, exited, kernel, user) },
                0
            );
            let ticks =
                |time: &FILETIME| ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64;
            ticks(kernel) + ticks(user)
        }
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        // Explicitly reproduce Winsock's inherited accept mode.
        stream.set_nonblocking(true).unwrap();
        client.write_all(b"GET / HTTP/1.1\r\n").unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            let cpu_before = cpu_time();
            let start = Instant::now();
            ready_tx.send(()).unwrap();
            relay(
                stream,
                "token",
                SID,
                &[],
                Path::new("C:/"),
                Path::new("C:/"),
                &BTreeMap::new(),
                &worker_stop,
                &Mutex::new(None),
                "fixture",
            )
            .unwrap();
            (start.elapsed(), cpu_time() - cpu_before)
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        thread::sleep(Duration::from_millis(400));
        stop.store(true, Ordering::Release);
        let (elapsed, cpu) = worker.join().unwrap();
        assert!(elapsed >= Duration::from_millis(350));
        assert!(elapsed < Duration::from_secs(2));
        assert!(cpu < 500_000, "idle relay consumed {}ms CPU", cpu / 10_000);
    }
    #[test]
    fn local_endpoint_rejects_wrong_tokens_and_other_tabs_cannot_close_it() {
        let endpoint = start(
            SID,
            &["never-launch-this".into()],
            Path::new("C:/"),
            Path::new("C:/"),
            &BTreeMap::new(),
        )
        .unwrap();
        let address = endpoint
            .split_whitespace()
            .next()
            .unwrap()
            .strip_prefix("ws://")
            .unwrap()
            .split('/')
            .next()
            .unwrap();
        close("other-tab", &endpoint);
        assert!(active(SID, &endpoint));
        assert!(!active("other-tab", &endpoint));
        // A partial handshake must not block another client (the TUI also
        // keeps a long-lived foreground connection while opening its picker).
        let mut slow = TcpStream::connect(address).unwrap();
        slow.write_all(b"GET / HTTP/1.1\r\n").unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer wrong\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(response.starts_with("HTTP/1.1 403 "));
        let mut stalled = vec![slow];
        for _ in 1..MAX_CONNECTIONS {
            let mut socket = TcpStream::connect(address).unwrap();
            socket.write_all(b"GET / HTTP/1.1\r\n").unwrap();
            stalled.push(socket);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let count = LINKS
                .lock()
                .unwrap()
                .get(&endpoint)
                .unwrap()
                .connections
                .lock()
                .unwrap()
                .len();
            if count == MAX_CONNECTIONS {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "auxiliary connections were serialized"
            );
            thread::sleep(Duration::from_millis(10));
        }
        let mut excess = TcpStream::connect(address).unwrap();
        excess
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        match excess.read(&mut [0]) {
            Ok(0) => {}
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
            result => panic!("excess connection was not rejected: {result:?}"),
        }
        close(SID, &endpoint);
        assert!(!active(SID, &endpoint));
        for mut socket in stalled {
            socket
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            assert_eq!(socket.read(&mut [0]).unwrap(), 0);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && TcpStream::connect(address).is_ok() {
            thread::sleep(Duration::from_millis(30));
        }
        assert!(TcpStream::connect(address).is_err());
    }
    #[test]
    fn binds_only_successful_frontend_requests_and_ignores_other_threads() {
        let mut observer = Observer::default();
        let done = |thread: &str, status: &str| json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":"turn1","status":status,"items":[{"text":"SECRET"}]}}});
        observer.request(&json!({"id":1,"method":"thread/resume","params":{"threadId":"a"}}));
        observer.response(&json!({"id":1,"error":{"code":-1}}), SID);
        assert!(observer.response(&done("a", "completed"), SID).is_none());
        observer.request(&json!({"id":2,"method":"thread/start"}));
        observer.response(
            &json!({"id":2,"result":{"thread":{"id":"a","cwd":"C:/work"}}}),
            SID,
        );
        assert!(observer
            .response(&done("child", "completed"), SID)
            .is_none());
        let event = observer.response(&done("a", "completed"), SID).unwrap();
        assert_eq!(event.state, State::Complete);
        assert_eq!(event.session_id, SID);
        assert!(!serde_json::to_string(&event).unwrap().contains("SECRET"));
        assert!(observer.response(&done("a", "completed"), SID).is_none());
        assert_eq!(
            observer.response(&done("a", "failed"), SID).unwrap().state,
            State::Failed
        );
        assert_eq!(
            observer
                .response(&done("a", "interrupted"), SID)
                .unwrap()
                .state,
            State::Interrupted
        );
        observer.request(&json!({"id":3,"method":"thread/unsubscribe","params":{"threadId":"a"}}));
        observer.response(&json!({"id":3,"result":{}}), SID);
        assert!(observer.response(&done("a", "completed"), SID).is_none());
    }
    #[test]
    fn masked_fragmented_messages_and_control_frames_survive_arbitrary_chunks() {
        fn frame(op: u8, text: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
            let mut bytes = vec![
                op,
                (text.len() as u8) | if mask.is_some() { 128 } else { 0 },
            ];
            if let Some(mask) = mask {
                bytes.extend(mask);
            }
            bytes.extend(
                text.iter()
                    .enumerate()
                    .map(|(i, &b)| b ^ mask.map_or(0, |m| m[i % 4])),
            );
            bytes
        }
        let mut bytes = frame(1, b"{\"method\":", Some([1, 2, 3, 4]));
        bytes.extend(frame(137, b"ping", None));
        bytes.extend(frame(128, b"\"turn/started\"}", Some([4, 3, 2, 1])));
        for chunk in 1..bytes.len() {
            let mut frames = Frames::default();
            let mut messages = Vec::new();
            for data in bytes.chunks(chunk) {
                frames.feed(data, |value| messages.push(value));
            }
            assert_eq!(messages, vec![json!({"method":"turn/started"})]);
        }
    }

    #[test]
    fn extended_lengths_and_oversized_messages_do_not_break_the_next_event() {
        let mut frames = Frames::default();
        let mut messages = Vec::new();
        let size = (MAX_METADATA + 10) as u64;
        let mut bytes = vec![0x81, 127];
        bytes.extend(size.to_be_bytes());
        frames.feed(&bytes, |value| messages.push(value));
        for _ in 0..MAX_METADATA / 1024 {
            frames.feed(&[b'x'; 1024], |value| messages.push(value));
        }
        frames.feed(&[b'x'; 10], |value| messages.push(value));
        assert!(messages.is_empty());
        assert!(frames.message.len() <= MAX_METADATA);
        let expected = json!({"method":"turn/completed","ignored":"x".repeat(512)});
        let payload = serde_json::to_vec(&expected).unwrap();
        let mut bytes = vec![0x81, 126];
        bytes.extend((payload.len() as u16).to_be_bytes());
        bytes.extend(payload);
        for data in bytes.chunks(17) {
            frames.feed(data, |value| messages.push(value));
        }
        assert_eq!(messages, vec![expected]);
    }
}
