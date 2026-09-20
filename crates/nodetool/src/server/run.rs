//! The run the editor starts and stops: the server state a run of the
//! held definition occupies, and the act that sets it running.
//!
//! The state is one of two: a run is on — carrying the channel a stop
//! travels through — or idle, carrying the last run's outcome, none
//! before any run. The start operation flips idle to running itself, the
//! act that causes the run; every ending arrives through the engine's own
//! event stream — the [`super::bridge::Bridge`] hears run finished and
//! carries its outcome back — so the endings are the run's own, told on
//! the engine's event stream, with no second tracker beside it. A
//! failure's node rides the outcome itself, named where the failing task
//! knew it, so what the failure names is what the engine named, not a
//! parsing of the message and not a hearing beside it.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;
use uuid::Uuid;

use crate::behaviour::Receiver;

/// The consumer of one tapped input's values, supplied by the binary
/// hosting the server: built over the stream the input delivers to it, one
/// stream per tapped node instance. The engine's tap mechanism carried
/// through the server, and what the consumer does with the values is the
/// host's — core names no node type, port, or plugin of its own.
pub type TapConsumer = Arc<dyn Fn(Receiver<crate::Value>) -> TapStream + Send + Sync>;

/// One tap's stream: every value the input receives, in order, then the
/// stream's end. Its error ends the run like any downstream's.
pub type TapStream = Pin<Box<dyn Future<Output = Result<(), crate::behaviour::Error>> + Send>>;

/// What a host listens to in the runs the editor starts: the node type
/// whose instances carry the input, the input port on them, and the
/// consumer built over each instance's stream. The host names its interest
/// by its own plugin's types; the server resolves the instances against
/// the graph each run compiles, so a node added, wired, or deleted in the
/// browser is tapped, or not, by the very next run.
#[derive(Clone)]
pub(super) struct Tap {
    pub(super) type_ref: &'static str,
    pub(super) port: &'static str,
    pub(super) consumer: TapConsumer,
}

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

/// Spawn the run of `compiled` the editor just started, with the channel a
/// stop arrives on attached and the bridge subscribed as the run's one
/// observer. Each tap the hosting binary named attaches to every instance
/// of its node type this graph carries. The compiled graph moves into the
/// task and dies with the run — no compiled graph is kept between runs.
/// The run's ending comes back through the bridge.
pub(super) fn spawn(
    session: Arc<Mutex<super::Session>>,
    pushes: tokio::sync::broadcast::Sender<String>,
    registry: std::sync::Arc<crate::registry::Registry>,
    compiled: crate::compile::CompiledGraph,
    stop_requested: watch::Receiver<bool>,
    taps: Vec<Tap>,
) {
    let bridge = Arc::new(super::bridge::Bridge::new(session, pushes, registry));
    tokio::spawn(async move {
        let mut run = crate::engine::Run::new(&compiled);
        run.stop_on(stop_requested);
        run.observe(bridge);
        for tap in &taps {
            let instances: Vec<Uuid> = compiled
                .nodes
                .iter()
                .filter(|(_, node)| node.node_type.type_ref == tap.type_ref)
                .map(|(uuid, _)| *uuid)
                .collect();
            for uuid in instances {
                run.tap(uuid, tap.port, {
                    let consumer = Arc::clone(&tap.consumer);
                    move |values| consumer(values)
                });
            }
        }
        // The outcome itself arrives through the bridge — the run's own
        // event stream — so the returned result is the same truth twice.
        let _ = run.start().await;
    });
}
