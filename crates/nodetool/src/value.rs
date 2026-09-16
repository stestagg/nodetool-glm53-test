//! The one runtime value representation: everything that travels on every
//! stream — base scalars, plugin custom types, parameter literals — carried
//! as a single value that names the data type it is an instance of.
//!
//! Core sees the identity and nothing more: the payload is erased, readable
//! as a concrete Rust type only by code that knows that type — the behaviour
//! of the plugin whose port declared it. A port declared as a union of types
//! carries values of any of its members through this one representation;
//! the behaviour reads each value as the concrete type it received.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use uuid::Uuid;

/// A value on a stream: the id of the data type it is an instance of, and
/// the erased payload only the declaring plugin reads.
#[derive(Clone)]
pub struct Value {
    type_id: Uuid,
    payload: Arc<dyn Any + Send + Sync>,
}

impl Value {
    /// A value of the data type named by `type_id`, carrying `payload`.
    /// Pairing a payload with a type whose payload shape it is not is an
    /// authoring bug: reads of it fail the way a bad cast does, at the
    /// behaviour that misreads it.
    pub fn new(type_id: Uuid, payload: impl Any + Send + Sync) -> Value {
        Value {
            type_id,
            payload: Arc::new(payload),
        }
    }

    /// The id of the data type this value is an instance of.
    pub fn type_id(&self) -> Uuid {
        self.type_id
    }

    /// Read the payload as the concrete Rust type it carries, if it is one.
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.payload.downcast_ref::<T>()
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The payload is erased: identity only.
        f.debug_struct("Value")
            .field("type_id", &self.type_id)
            .finish()
    }
}
