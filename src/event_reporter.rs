//! Deliver the latest advisory status without blocking the transport producing it.
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    thread,
};

struct State<T> {
    pending: Option<T>,
    stopped: bool,
}
struct Shared<T> {
    state: Mutex<State<T>>,
    wake: Condvar,
}

pub(crate) struct Reporter<T> {
    shared: Arc<Shared<T>>,
}
impl<T: Send + 'static> Reporter<T> {
    pub fn start(mut report: impl FnMut(T) + Send + 'static) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                pending: None,
                stopped: false,
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        thread::Builder::new()
            .name("codex-status".into())
            .spawn(move || loop {
                let value = {
                    let mut state = worker.state.lock().unwrap_or_else(|p| p.into_inner());
                    while !state.stopped && state.pending.is_none() {
                        state = worker.wake.wait(state).unwrap_or_else(|p| p.into_inner());
                    }
                    if state.stopped {
                        break;
                    }
                    state.pending.take().unwrap()
                };
                report(value);
            })?;
        Ok(Self { shared })
    }

    pub fn report(&self, value: T) {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        if !state.stopped {
            // Status is advisory: preserve the newest state if IPC is slower
            // than the stream, using at most one pending and one in-flight event.
            state.pending = Some(value);
            self.shared.wake.notify_one();
        }
    }
}
impl<T> Drop for Reporter<T> {
    fn drop(&mut self) {
        let mut state = self.shared.state.lock().unwrap_or_else(|p| p.into_inner());
        state.stopped = true;
        state.pending = None;
        self.shared.wake.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn blocked_status_ipc_coalesces_to_latest_and_drop_discards_pending() {
        let (arrived, events) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let (done, finished) = mpsc::channel();
        struct Finish(mpsc::Sender<()>);
        impl Drop for Finish {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let finish = Finish(done);
        let reporter = Reporter::start(move |event: u32| {
            let _ = &finish;
            arrived.send(event).unwrap();
            gate.recv().unwrap();
        })
        .unwrap();
        reporter.report(1);
        assert_eq!(events.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
        for event in 2..=100_000 {
            reporter.report(event);
        }
        release.send(()).unwrap();
        assert_eq!(
            events.recv_timeout(Duration::from_secs(1)).unwrap(),
            100_000
        );
        reporter.report(100_001);
        drop(reporter);
        release.send(()).unwrap();
        finished.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(events.try_recv().is_err());
    }
}
