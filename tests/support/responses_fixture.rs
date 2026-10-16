//! Local deterministic SSE transport fixture. It never calls a model service.
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct ResponsesFixture {
    pub address: String,
    pub requests: Arc<AtomicUsize>,
    pub captured: Arc<Mutex<Vec<Value>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl ResponsesFixture {
    pub fn start(text: &str) -> Self {
        Self::start_inner(text, None)
    }
    pub fn start_shell(command: &str, workdir: &str) -> Self {
        Self::start_inner(
            "__BASH_FIXTURE_COMPLETED__",
            Some(
                json!({"cmd":command,"workdir":workdir,"shell":"bash","login":false,"max_output_tokens":2000}),
            ),
        )
    }
    fn start_inner(text: &str, shell: Option<Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let captured = Arc::new(Mutex::new(Vec::new()));
        let worker_captured = captured.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let worker_requests = requests.clone();
        let worker_stop = stop.clone();
        let text = text.to_owned();
        let worker = thread::spawn(move || {
            let mut clients: Vec<JoinHandle<()>> = Vec::new();
            while !worker_stop.load(Ordering::Acquire) {
                for index in (0..clients.len()).rev() {
                    if clients[index].is_finished() {
                        clients.swap_remove(index).join().unwrap();
                    }
                }
                if clients.len() == 8 {
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let requests = worker_requests.clone();
                        let text = text.clone();
                        let shell = shell.clone();
                        let captured = worker_captured.clone();
                        // An idle preconnected socket must not block a different
                        // connection carrying the actual SSE request.
                        clients.push(thread::spawn(move || {
                            match serve(stream, &text, shell.as_ref(), &captured) {
                                Ok(true) => {
                                    requests.fetch_add(1, Ordering::Release);
                                }
                                Ok(false) => {}
                                Err(error) => {
                                    eprintln!("Local SSE fixture transport error: {error}")
                                }
                            }
                        }));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
            for client in clients {
                client.join().unwrap();
            }
        });
        Self {
            address,
            requests,
            captured,
            stop,
            worker: Some(worker),
        }
    }
}

#[test]
fn idle_connection_does_not_block_sse_request() {
    let fixture = ResponsesFixture::start("__IDLE_TRANSPORT__");
    let endpoint = fixture
        .address
        .trim_start_matches("http://")
        .trim_end_matches("/v1");
    let idle = TcpStream::connect(endpoint).unwrap();
    let mut active = TcpStream::connect(endpoint).unwrap();
    active
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    active.write_all(b"POST /v1/responses HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
    let mut response = String::new();
    active.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(response.contains("__IDLE_TRANSPORT__"));
    drop(idle);
}
impl Drop for ResponsesFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn serve(
    mut stream: TcpStream,
    text: &str,
    shell: Option<&Value>,
    captured: &Mutex<Vec<Value>>,
) -> std::io::Result<bool> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut bytes = Vec::new();
    let mut buffer = [0; 8192];
    let header_end = loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 || bytes.len() + count > 4 * 1024 * 1024 {
            return Ok(false);
        }
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let response_request = headers.starts_with("POST /v1/responses ");
    let length = headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    if length > 4 * 1024 * 1024 {
        return Ok(false);
    }
    while bytes.len() < header_end + length {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            return Ok(false);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    if !response_request {
        stream.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )?;
        return Ok(false);
    }
    // Only the isolated Bash fixture captures bodies; normal notification
    // fixtures still drain and discard them. Nothing is forwarded to a service.
    let tool_call = if let Some(shell) = shell {
        let request: Value = serde_json::from_slice(&bytes[header_end..header_end + length])?;
        let has_result = request
            .get("input")
            .and_then(Value::as_array)
            .is_some_and(|items| {
                items.iter().any(|item| {
                    item.get("type").and_then(Value::as_str) == Some("function_call_output")
                })
            });
        captured.lock().unwrap().push(request);
        (!has_result).then(|| json!({"id":"fc_bash_fixture","type":"function_call","call_id":"call_bash_fixture","name":"exec_command","arguments":shell.to_string(),"status":"completed"}))
    } else {
        None
    };
    let part = json!({"type":"output_text","text":text,"annotations":[],"logprobs":[]});
    let item = json!({"id":"msg_miniterm_fixture","type":"message","role":"assistant","status":"completed","content":[part]});
    let events = if let Some(item) = tool_call {
        vec![
            json!({"type":"response.created","response":{"id":"resp_bash_fixture","object":"response","status":"in_progress","output":[]}}),
            json!({"type":"response.output_item.added","output_index":0,"item":item}),
            json!({"type":"response.output_item.done","output_index":0,"item":item}),
            json!({"type":"response.completed","response":{"id":"resp_bash_fixture","object":"response","status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}),
        ]
    } else {
        vec![
            json!({"type":"response.created","response":{"id":"resp_miniterm_fixture","object":"response","created_at":1791695400,"model":"miniterm-notify-fixture","status":"in_progress","output":[]}}),
            json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_miniterm_fixture","type":"message","role":"assistant","status":"in_progress","content":[]}}),
            json!({"type":"response.content_part.added","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","part":{"type":"output_text","text":"","annotations":[],"logprobs":[]}}),
            json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","delta":text,"logprobs":[]}),
            json!({"type":"response.output_text.done","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","text":text,"logprobs":[]}),
            json!({"type":"response.content_part.done","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","part":part}),
            json!({"type":"response.output_item.done","output_index":0,"item":item}),
            json!({"type":"response.completed","response":{"id":"resp_miniterm_fixture","object":"response","created_at":1791695400,"model":"miniterm-notify-fixture","status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}}}),
        ]
    };
    let mut body = String::new();
    for (sequence, mut event) in events.into_iter().enumerate() {
        event["sequence_number"] = json!(sequence);
        body.push_str(&format!(
            "event: {}\ndata: {}\n\n",
            event["type"].as_str().unwrap(),
            event
        ));
    }
    let headers = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
    stream.write_all(headers.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;
    Ok(true)
}
