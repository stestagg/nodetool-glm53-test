//! The bridge: the run's one observer into the editor session — story
//! 07's event stream composed as a single subscriber, speaking over the
//! push channel to every connection.
//!
//! It does three things with each event, in order. It forwards the event
//! itself, untouched in its browser form — every event, values included,
//! no sampling, thinning, or aggregation; the editor is not to be less
//! truthful than the headless terminal's printing observer. It derives the
//! node status the events define — running from a node's started event,
//! completed or failed from its own final transition, and stopped when a
//! failed or user-stopped run-finished closes a started node without one —
//! once, here, keyed by instance uuid, and pushes each derived change as
//! state. And it holds the latest value per emitting port, a base scalar
//! as its plain string form, a plugin custom type as nothing at all —
//! rendering those is plugin territory, core staying opaque to them.
//!
//! The statuses and the values persist after the run ends — the failure
//! stays locatable, the counter's last ticked value is evidence of what
//! the run did — until the next start resets them, and travel the
//! connect-time snapshot beside the run state, so a browser connecting at
//! any time sees the canvas the first tab sees.
//!
//! The run never waits on any of this: an observer is a sink, and the
//! push channel is bounded per connection — a connection that cannot keep
//! up is dropped rather than the transport thinning or the run waiting.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use serde_json::json;
use uuid::Uuid;

use super::protocol;
use super::run::{Outcome, RunState};
use super::Session;
use crate::async_trait;
use crate::engine::{Event, Observer, RunOutcome};
use crate::Value;

/// A node's derived status: story 07's, extended with the stopped outcome
/// a failed or user-stopped run leaves behind. Rendered as its name; the
/// canvas invents no states of its own.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Status {
    Running,
    Completed,
    Failed,
    Stopped,
}

impl Status {
    fn name(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Completed => "completed",
            Status::Failed => "failed",
            Status::Stopped => "stopped",
        }
    }
}

/// What the canvas shows of a run, held beside the session state it
/// belongs to: the per-node derived status and the latest value held per
/// emitting port. Ordered maps, so the snapshot every connection renders
/// reads in the same order.
#[derive(Default)]
pub(super) struct RunDisplay {
    statuses: HashMap<Uuid, Status>,
    values: BTreeMap<(Uuid, &'static str), String>,
}

impl RunDisplay {
    /// The next start resets the canvas: the last run's statuses and
    /// values give way as the new run's own events arrive.
    pub(super) fn reset(&mut self) {
        self.statuses.clear();
        self.values.clear();
    }

    /// The snapshot the connect-time resync carries beside the run state:
    /// the current displayed truth, mid-run's or the persisted last run's.
    pub(super) fn snapshot(&self) -> serde_json::Value {
        json!({
            "statuses": self
                .statuses
                .iter()
                .map(|(uuid, status)| (uuid.to_string(), status.name()))
                .collect::<BTreeMap<_, _>>(),
            "values": self
                .values
                .iter()
                .map(|((uuid, port), text)| {
                    json!({ "node": uuid, "port": port, "value": text })
                })
                .collect::<Vec<_>>(),
        })
    }
}

/// The engine observer one run is watched by: it forwards every event,
/// derives the statuses, holds the values, and carries the run-finished
/// outcome back into the run state — the whole session-side hearing of a
/// run, behind one observer.
pub(super) struct Bridge {
    session: Arc<Mutex<Session>>,
    pushes: tokio::sync::broadcast::Sender<String>,
}

impl Bridge {
    pub(super) fn new(
        session: Arc<Mutex<Session>>,
        pushes: tokio::sync::broadcast::Sender<String>,
    ) -> Bridge {
        Bridge { session, pushes }
    }

    /// Push one derived status change as state.
    fn push_status(&self, uuid: Uuid, status: Status) {
        let _ = self
            .pushes
            .send(protocol::node_status_message(uuid, status.name()));
    }
}

#[async_trait]
impl Observer for Bridge {
    async fn observe(&self, event: Event) {
        self.hear(event);
        // One beat back to the scheduler between events: a burst of
        // emissions drains through here in a single poll, and the
        // per-connection pumps need a slot to keep pace with it. The run
        // never waits — the event queue absorbs whatever this costs.
        tokio::task::yield_now().await;
    }
}

impl Bridge {
    /// The whole hearing of one event, under the session lock: forward it,
    /// derive what it changes, push the change as state.
    fn hear(&self, event: Event) {
        let mut session = self
            .session
            .lock()
            .expect("the session lock is never poisoned");

        // An emission's browser-renderable text: held as the port's latest,
        // and riding the forwarded event. A plugin custom type renders no
        // content of core's inventing — the event still crosses, the wire
        // still animates.
        let value = match &event {
            Event::Emitted {
                node,
                port,
                value: emitted,
            } => browser_value(emitted).inspect(|text| {
                session
                    .display
                    .values
                    .insert((node.uuid, port), text.clone());
            }),
            _ => None,
        };
        let _ = self
            .pushes
            .send(protocol::run_event_message(&event, value.as_deref()));

        match event {
            Event::RunStarted | Event::Emitted { .. } => {}
            Event::NodeStarted { node } => {
                session.display.statuses.insert(node.uuid, Status::Running);
                self.push_status(node.uuid, Status::Running);
            }
            Event::NodeCompleted { node } => {
                session
                    .display
                    .statuses
                    .insert(node.uuid, Status::Completed);
                self.push_status(node.uuid, Status::Completed);
            }
            Event::NodeFailed { node, .. } => {
                session.display.statuses.insert(node.uuid, Status::Failed);
                self.push_status(node.uuid, Status::Failed);
            }
            Event::RunFinished { outcome } => {
                // Story 07's closure rule, extended to the stopped outcome:
                // a failed or user-stopped run closes every started node
                // without its own final transition as stopped — the
                // abandoned work, told on the events, with no per-node
                // aborted event invented to render.
                let outcome = match outcome {
                    RunOutcome::Complete => Outcome::Completed,
                    RunOutcome::Failed { error, node } => Outcome::Failed {
                        error,
                        node: node.map(|node| node.uuid),
                    },
                    RunOutcome::Stopped => Outcome::Stopped,
                };
                if !matches!(outcome, Outcome::Completed) {
                    let abandoned: Vec<Uuid> = session
                        .display
                        .statuses
                        .iter()
                        .filter(|(_, status)| **status == Status::Running)
                        .map(|(uuid, _)| *uuid)
                        .collect();
                    for uuid in abandoned {
                        session.display.statuses.insert(uuid, Status::Stopped);
                        self.push_status(uuid, Status::Stopped);
                    }
                }
                session.run = RunState::Idle {
                    outcome: Some(outcome),
                };
                let _ = self.pushes.send(protocol::run_message(&session.run));
            }
        }
    }
}

/// A value's reading on the canvas: the base scalars core ships render
/// their plain string form — the same text the headless runner prints —
/// and any other type is a plugin's custom type, whose rendering is that
/// plugin's business.
fn browser_value(value: &Value) -> Option<String> {
    if !crate::scalars::is_base_scalar(value.type_id()) {
        return None;
    }
    if let Some(text) = value.get::<String>() {
        return Some(text.clone());
    }
    macro_rules! scalars {
        ($($ty:ty),* $(,)?) => {$(
            if let Some(text) = value.get::<$ty>().map(ToString::to_string) {
                return Some(text);
            }
        )*};
    }
    scalars!(bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
    None
}
