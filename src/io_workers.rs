//! Cancel owned synchronous pipe operations without joining on the GUI thread.
use std::{
    os::windows::io::AsRawHandle,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::Win32::System::IO::CancelSynchronousIo;

#[derive(Default)]
pub(crate) struct Workers {
    stopped: AtomicBool,
    handles: Mutex<Vec<JoinHandle<()>>>,
}

impl Workers {
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    pub fn register(&self, worker: JoinHandle<()>) {
        let mut handles = self.handles.lock().unwrap_or_else(|p| p.into_inner());
        if self.stopped() {
            drop(handles);
            reap(vec![worker]);
        } else {
            handles.push(worker);
        }
    }

    pub fn stop(&self) -> bool {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return false;
        }
        let handles = std::mem::take(&mut *self.handles.lock().unwrap_or_else(|p| p.into_inner()));
        if !handles.is_empty() {
            reap(handles);
        }
        true
    }
}

fn reap(handles: Vec<JoinHandle<()>>) {
    fn cancel(handles: &[JoinHandle<()>]) {
        for worker in handles.iter().filter(|worker| !worker.is_finished()) {
            // These are handles of our own Rust threads, never a user's process.
            unsafe {
                CancelSynchronousIo(worker.as_raw_handle() as _);
            }
        }
    }
    cancel(&handles);
    let _ = thread::Builder::new()
        .name("pty-io-reaper".into())
        .spawn(move || {
            // Repeat to cover cancellation racing the check immediately before ReadFile.
            // No permanent polling thread is present during normal operation.
            let deadline = Instant::now() + Duration::from_secs(2);
            while handles.iter().any(|worker| !worker.is_finished()) && Instant::now() < deadline {
                cancel(&handles);
                thread::sleep(Duration::from_millis(10));
            }
            for worker in handles {
                if worker.is_finished() {
                    let _ = worker.join();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::windows::io::FromRawHandle,
        sync::{mpsc, Arc},
    };
    use windows_sys::Win32::System::Pipes::CreatePipe;

    #[test]
    fn stop_cancels_a_blocked_owned_pipe_reader_without_waiting_for_the_writer() {
        let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(
            unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) },
            0
        );
        let mut read = unsafe { std::fs::File::from_raw_handle(read as _) };
        let _write = unsafe { std::fs::File::from_raw_handle(write as _) };
        let workers = Arc::new(Workers::default());
        let (ready, started) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        workers.register(thread::spawn(move || {
            ready.send(()).unwrap();
            let result = read.read(&mut [0]);
            done.send(result.is_err()).unwrap();
        }));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(workers.stop());
        assert!(!workers.stop());
        assert!(finished.recv_timeout(Duration::from_secs(3)).unwrap());
        // The writer stays open throughout; EOF cannot make this test pass.
    }
}
