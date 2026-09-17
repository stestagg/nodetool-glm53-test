//! The generic-port idiom, plugin-side: the numeric family the fizzbuzz
//! nodes are written against.
//!
//! The author writes each node's numeric logic once, generic over
//! [`Numeric`] — the arithmetic every member of the base numeric set
//! shares. Two helpers stamp the per-type code from that one body:
//! [`NUMERICS`] is the member list the family ports declare, and
//! [`for_numeric!`] picks the stamped instance for the type the compiler
//! resolved the family to. Nothing repeats per type: one generic body, one
//! stamping site.

use nodetool::behaviour::Io;
use nodetool::{Uuid, Value};

/// The arithmetic and identity every member of the numeric family shares.
/// Comparisons ride the standard `PartialEq`/`PartialOrd` the base scalars
/// already carry.
pub trait Numeric: Copy + Send + Sync + PartialEq + PartialOrd + 'static {
    /// The data type reference this member is registered under.
    const TYPE_REF: &'static str;
    /// The data type id this member's values carry.
    const TYPE_ID: Uuid;
    /// The member's zero.
    fn zero() -> Self;
    /// The next step of the sequence, or none when the arithmetic runs out
    /// of room — an exhausted accumulator has passed any stop.
    fn add_checked(self, step: Self) -> Option<Self>;
}

macro_rules! integers {
    ($($ty:ty => $id:path),+ $(,)?) => {$(
        impl Numeric for $ty {
            const TYPE_REF: &'static str = stringify!($ty);
            const TYPE_ID: Uuid = $id;
            fn zero() -> Self { 0 }
            fn add_checked(self, step: Self) -> Option<Self> { self.checked_add(step) }
        }
    )*};
}

integers! {
    i8 => nodetool::scalars::I8,
    i16 => nodetool::scalars::I16,
    i32 => nodetool::scalars::I32,
    i64 => nodetool::scalars::I64,
    u8 => nodetool::scalars::U8,
    u16 => nodetool::scalars::U16,
    u32 => nodetool::scalars::U32,
    u64 => nodetool::scalars::U64,
}

macro_rules! floats {
    ($($ty:ty => $id:path),+ $(,)?) => {$(
        impl Numeric for $ty {
            const TYPE_REF: &'static str = stringify!($ty);
            const TYPE_ID: Uuid = $id;
            fn zero() -> Self { 0.0 }
            fn add_checked(self, step: Self) -> Option<Self> {
                // An overflowing float accumulates to an infinity, which no
                // stop in either direction contains; the sequence test ends it.
                Some(self + step)
            }
        }
    )*};
}

floats! {
    f32 => nodetool::scalars::F32,
    f64 => nodetool::scalars::F64,
}

/// The base numeric set, in the family's declaration order — the member
/// list the numeric ports declare, and the order the deterministic
/// tiebreak walks.
pub const NUMERICS: &[&str] = &[
    <i8 as Numeric>::TYPE_REF,
    <i16 as Numeric>::TYPE_REF,
    <i32 as Numeric>::TYPE_REF,
    <i64 as Numeric>::TYPE_REF,
    <u8 as Numeric>::TYPE_REF,
    <u16 as Numeric>::TYPE_REF,
    <u32 as Numeric>::TYPE_REF,
    <u64 as Numeric>::TYPE_REF,
    <f32 as Numeric>::TYPE_REF,
    <f64 as Numeric>::TYPE_REF,
];

/// The name the numeric ports declare their family under.
pub const NUMERIC_FAMILY: &str = "numeric";

/// Stamp the one generic body for the concrete member a compiled node's
/// family resolved to. `$build` is a generic function taking the compiled
/// node; its return type is whatever the caller needs — a behaviour, a
/// verdict.
macro_rules! for_numeric {
    ($compiled:expr, $build:ident) => {{
        let resolved = $compiled
            .families
            .get(NUMERIC_FAMILY)
            .expect("the compiler resolved every family a node declares");
        match resolved.name {
            "i8" => $build::<i8>($compiled),
            "i16" => $build::<i16>($compiled),
            "i32" => $build::<i32>($compiled),
            "i64" => $build::<i64>($compiled),
            "u8" => $build::<u8>($compiled),
            "u16" => $build::<u16>($compiled),
            "u32" => $build::<u32>($compiled),
            "u64" => $build::<u64>($compiled),
            "f32" => $build::<f32>($compiled),
            "f64" => $build::<f64>($compiled),
            other => panic!(
                "the `{}` family resolved to `{other}`, which the numeric set does not span",
                NUMERIC_FAMILY
            ),
        }
    }};
}

/// Whether the accumulated `value` may still grow toward `stop` along
/// `step`: not passed in step's direction, stop included when the
/// accumulation lands exactly on it. A zero step never moves, so it never
/// passes a stop on either side — the callers reject it first.
pub fn sequence_continues<T: Numeric>(value: T, stop: T, step: T) -> bool {
    if step <= T::zero() {
        value >= stop
    } else {
        value <= stop
    }
}

/// The input's current value as the member the family resolved to: the run
/// is gated on the input's first value, and the compiler resolved the
/// family to the type the behaviour is stamped for.
pub fn numeric_input<T: Numeric>(io: &mut Io<'_>, name: &str) -> T {
    io.input(name)
        .current()
        .and_then(|value| value.get::<T>().copied())
        .expect("the run is gated on this input's first value")
}

/// The value as a member of the numeric family, for the behaviour that
/// emits one.
pub fn numeric_value<T: Numeric>(value: T) -> Value {
    Value::new(T::TYPE_ID, value)
}

pub(crate) use for_numeric;
