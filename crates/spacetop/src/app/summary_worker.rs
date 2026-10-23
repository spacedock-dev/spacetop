use super::summary::{SummaryCompletion, SummaryRequest};
use super::App;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
use std::thread::{self, JoinHandle};

struct Running {
    nonce: u64,
    key: spacetop_core::summary::snapshot::SummaryKey,
    cancel: Arc<AtomicBool>,
    receiver: Receiver<SummaryCompletion>,
    join: JoinHandle<()>,
}
#[derive(Default)]
pub struct SummaryWorker {
    running: Option<Running>,
}
impl SummaryWorker {
    pub fn tick(&mut self, app: &mut App) {
        if app.should_quit() {
            if let Some(running) = &self.running {
                running.cancel.store(true, Ordering::Release);
            }
            return;
        }
        if let Some(running) = &self.running {
            if app.summary.active_nonce() != Some(running.nonce) {
                running.cancel.store(true, Ordering::Release);
            }
            if let Ok(result) = running.receiver.try_recv() {
                app.complete_summary(result);
            }
        }
        if self
            .running
            .as_ref()
            .is_some_and(|running| running.join.is_finished())
        {
            if let Some(running) = self.running.take() {
                let joined = running.join.join();
                if let Ok(result) = running.receiver.try_recv() {
                    app.complete_summary(result);
                } else if joined.is_err() {
                    app.complete_summary(SummaryCompletion {
                        nonce: running.nonce,
                        key: running.key,
                        result: Err(spacetop_core::summary::SummaryError::Process),
                    });
                }
            }
        }
        if self.running.is_none() {
            if let Some(request) = app.summary.take_request() {
                match spawn(request) {
                    Ok(running) => self.running = Some(running),
                    Err(completion) => app.complete_summary(*completion),
                }
            }
        }
    }
}
impl Drop for SummaryWorker {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.cancel.store(true, Ordering::Release);
            let _ = running.join.join();
        }
    }
}
fn spawn(request: SummaryRequest) -> Result<Running, Box<SummaryCompletion>> {
    spawn_with(request, |request, cancel| {
        spacetop_core::summary::generate(
            request.snapshot.key.provider,
            &request.snapshot.input,
            cancel,
        )
    })
}

fn spawn_with(
    request: SummaryRequest,
    generate: impl FnOnce(&SummaryRequest, &AtomicBool) -> Result<String, spacetop_core::summary::SummaryError>
        + Send
        + 'static,
) -> Result<Running, Box<SummaryCompletion>> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let (sender, receiver) = mpsc::channel();
    let nonce = request.nonce;
    let key = request.snapshot.key.clone();
    let join = thread::Builder::new()
        .name("spacetop-summary".into())
        .spawn(move || {
            let result = generate(&request, &worker_cancel);
            let _ = sender.send(SummaryCompletion {
                nonce,
                key: request.snapshot.key,
                result,
            });
        })
        .map_err(|_| {
            Box::new(SummaryCompletion {
                nonce,
                key: key.clone(),
                result: Err(spacetop_core::summary::SummaryError::Process),
            })
        })?;
    Ok(Running {
        nonce,
        key,
        cancel,
        receiver,
        join,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::summary::tests::{fixture, key};
    use crossterm::event::KeyCode;
    #[test]
    fn worker_drop_cancels_and_joins_without_live_provider() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        let request = app.summary.take_request().unwrap();
        let finished = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&finished);
        let running = spawn_with(request, move |_, cancel| {
            let mut command = std::process::Command::new("/bin/sh");
            command.args(["-c", "sleep 30"]);
            let result =
                spacetop_core::summary::process::run(command, b"", cancel, Default::default());
            observed.store(true, Ordering::Release);
            result.map(|_| String::new())
        })
        .unwrap_or_else(|_| panic!("worker spawn"));
        let worker = SummaryWorker {
            running: Some(running),
        };
        drop(worker);
        assert!(finished.load(Ordering::Acquire));
    }
    #[test]
    fn quit_does_not_launch_pending_generation() {
        let (mut app, _dir) = fixture();
        key(&mut app, KeyCode::Char('S'));
        key(&mut app, KeyCode::Enter);
        key(&mut app, KeyCode::Char('q'));
        key(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit());
        let mut worker = SummaryWorker::default();
        worker.tick(&mut app);
        assert!(worker.running.is_none());
    }
}
