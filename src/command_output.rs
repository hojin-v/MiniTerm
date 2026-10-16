//! Drain a background command's stderr with a small tail and cancellable reads.
use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{Arc, Mutex},
    thread,
};

const TAIL_BYTES: usize = 4096;
pub(crate) struct ErrorOutput {
    tail: Arc<Mutex<VecDeque<u8>>>,
    workers: crate::io_workers::Workers,
}
impl ErrorOutput {
    pub fn capture(mut reader: impl Read + Send + 'static) -> io::Result<Self> {
        let tail = Arc::new(Mutex::new(VecDeque::with_capacity(TAIL_BYTES)));
        let output = tail.clone();
        let workers = crate::io_workers::Workers::default();
        workers.register(thread::Builder::new().name("command-stderr".into()).spawn(
            move || {
                let mut buffer = [0; 1024];
                while let Ok(count) = reader.read(&mut buffer) {
                    if count == 0 {
                        break;
                    }
                    let mut tail = output.lock().unwrap_or_else(|p| p.into_inner());
                    let discard = (tail.len() + count).saturating_sub(TAIL_BYTES);
                    tail.drain(..discard);
                    tail.extend(&buffer[..count]);
                }
            },
        )?);
        Ok(Self { tail, workers })
    }
    pub fn message(&self) -> String {
        let tail = self.tail.lock().unwrap_or_else(|p| p.into_inner());
        let bytes: Vec<_> = tail.iter().copied().collect();
        String::from_utf8_lossy(&bytes)
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .collect::<String>()
            .trim()
            .to_owned()
    }
}
impl Drop for ErrorOutput {
    fn drop(&mut self) {
        self.workers.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn long_command_errors_are_drained_and_only_a_bounded_tail_is_retained() {
        let mut bytes = vec![b'x'; 100_000];
        bytes.extend_from_slice(b"\nLAST-ERROR\n");
        let output = ErrorOutput::capture(io::Cursor::new(bytes)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !output.message().ends_with("LAST-ERROR") {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert!(output.tail.lock().unwrap().len() <= TAIL_BYTES);
        assert!(!output.message().contains('\0'));
    }
}
