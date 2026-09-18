//! The run the editor starts and stops: the server state a run of the
//! held definition occupies, and the observer that hears the run's
//! endings.
//!
//! The state is one of two: a run is on — carrying the channel a stop
//! travels through — or idle, carrying the last run's outcome, none
//! before any run. The start operation flips idle to running itself, the
//! act that causes the run; every ending arrives through the engine's own
//! event stream — [`RunWatcher`] hears run finished and carries its
//! outcome back — so the endings are the run's own, told on the engine's
//! event stream, with no second tracker beside it. A failure's node rides
//! the outcome itself, named where the failing task knew it, so what the
//! failure names is what the engine named, not a parsing of the message
//! and not a hearing beside it.

use std::sync::{Arc, Mutex};

use tokio::sync::watch;
use uuid::Uuid;

use super::protocol;
use crate::async_trait;
use crate::engine::{Event, Observer, RunOutcome};

/// How the last run ended: every node complete, a stop asked from the
/// chrome, or the first error, named — with the node instance whose
/// failure ended the run, when the run ended on one. An idle run with no
/// outcome has not run yet.
pub enum Outcome {
    Completed,
    Failed { error: String, node: Option<Uuid> },
    Stopped,
}

impl Outcome {
    /// The outcome's name, as the run state carries it beside the idle
    /// state.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::Stopped => "stopped",
            Outcome::Failed { .. } => "failed",
        }
    }
}

/// Whether a run is on, and when idle, how the last one ended. Running
/// carries the stop channel: the `true` a stop sends on it is the engine's
/// signal to end the run.
pub enum RunState {
    Running { stop: watch::Sender<bool> },
    Idle { outcome: Option<Outcome> },
}

impl RunState {
    /// Whether a run is on — the one fact every editing operation is
    /// judged under, and the control's flip.
    pub fn running(&self) -> bool {
        matches!(self, RunState::Running { .. })
    }
}

/// The engine observer one run is watched by: it carries the run's
/// finished outcome back into the run state and pushes the change to every
/// connection. The running state is the start operation's own doing, so
/// run-finished is all this watcher answers to.
struct RunWatcher {
    session: Arc<Mutex<super::Session>>,
    pushes: tokio::sync::broadcast::Sender<String>,
}

#[async_trait]
impl Observer for RunWatcher {
    async fn observe(&self, event: Event) {
        // The running state is the start operation's own doing, so
        // run-finished is all this watcher answers to.
        if let Event::RunFinished { outcome } = event {
            let outcome = match outcome {
                RunOutcome::Complete => Outcome::Completed,
                RunOutcome::Failed { error, node } => Outcome::Failed {
                    error,
                    node: node.map(|node| node.uuid),
                },
                RunOutcome::Stopped => Outcome::Stopped,
            };
            let mut session = self
                .session
                .lock()
                .expect("the session lock is never poisoned");
            session.run = RunState::Idle {
                outcome: Some(outcome),
            };
            let _ = self.pushes.send(protocol::run_message(&session.run));
        }
    }
}

/// Spawn the run of `compiled` the editor just started, with the channel a
/// stop arrives on attached. The compiled graph moves into the task and
/// dies with the run — no compiled graph is kept between runs. The run's
/// ending comes back through [`RunWatcher`].
pub(super) fn spawn(
    session: Arc<Mutex<super::Session>>,
    pushes: tokio::sync::broadcast::Sender<String>,
    compiled: crate::compile::CompiledGraph,
    stop_requested: watch::Receiver<bool>,
) {
    let watcher = Arc::new(RunWatcher { session, pushes });
    tokio::spawn(async move {
        let mut run = crate::engine::Run::new(&compiled);
        run.stop_on(stop_requested);
        run.observe(watcher);
        // The outcome itself arrives through the watcher — the run's own
        // event stream — so the returned result is the same truth twice.
        let _ = run.start().await;
    });
}
