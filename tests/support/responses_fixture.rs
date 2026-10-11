//! Local deterministic SSE transport fixture. It never calls a model service.
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub struct ResponsesFixture {
    pub address: String,
    pub requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl ResponsesFixture {
    pub fn start(text: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = format!("http://{}/v1", listener.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_requests = requests.clone();
        let worker_stop = stop.clone();
        let text = text.to_owned();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if serve(stream, &text).is_ok_and(|served| served) {
                            worker_requests.fetch_add(1, Ordering::Release);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            requests,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for ResponsesFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn serve(mut stream: TcpStream, text: &str) -> std::io::Result<bool> {
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
    // Input bodies are drained and discarded, never logged or forwarded.
    let part = json!({"type":"output_text","text":text,"annotations":[],"logprobs":[]});
    let item = json!({"id":"msg_miniterm_fixture","type":"message","role":"assistant","status":"completed","content":[part]});
    let events = [
        json!({"type":"response.created","response":{"id":"resp_miniterm_fixture","object":"response","created_at":1791695400,"model":"miniterm-notify-fixture","status":"in_progress","output":[]}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_miniterm_fixture","type":"message","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","part":{"type":"output_text","text":"","annotations":[],"logprobs":[]}}),
        json!({"type":"response.output_text.delta","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","delta":text,"logprobs":[]}),
        json!({"type":"response.output_text.done","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","text":text,"logprobs":[]}),
        json!({"type":"response.content_part.done","output_index":0,"content_index":0,"item_id":"msg_miniterm_fixture","part":part}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":{"id":"resp_miniterm_fixture","object":"response","created_at":1791695400,"model":"miniterm-notify-fixture","status":"completed","output":[item],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}}}),
    ];
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
