//! The data type model: the shared vocabulary every port's type reference
//! names.
//!
//! A data type is one descriptor, [`DataType`], whoever declares it: core
//! declares the base scalars (see [`scalars`]), plugins declare their own
//! custom types with [`data_type!`]. Beyond the descriptor a type is opaque —
//! core neither knows nor cares what a custom type's values look like, or how
//! they are serialised and rendered; that stays the declaring plugin's
//! business.

use std::fmt;

use uuid::Uuid;

use crate::Value;

/// A data type as its declarer contributes it: pure data, identified by a
/// stable [`DataType::id`] and a unique [`DataType::name`] — the name being
/// the reference ports carry — optionally declaring conversions to other
/// types and carrying metadata.
#[derive(Clone, Copy, Debug)]
pub struct DataType {
    /// Stable identity, unique across the registered types.
    pub id: Uuid,
    /// Unique name; the reference ports carry (scalars use their Rust names).
    pub name: &'static str,
    /// Conversions declared to other types, each with the function that
    /// performs it.
    pub conversions: &'static [Conversion],
    /// Free-form metadata: a map of plain values, printable and serialisable
    /// to the browser.
    pub meta: &'static [(&'static str, MetaValue)],
}

/// A conversion a [`DataType`] declares to another type: the target's id and
/// the function that performs it.
#[derive(Clone, Copy, Debug)]
pub struct Conversion {
    /// The id of the type this conversion produces.
    pub target: Uuid,
    pub convert: ConvertFn,
}

/// The signature of a [`Conversion`] function, written against the runtime
/// value ([`Value`]): the declarer reads and produces erased values as it
/// pleases — core sees none of their shapes.
pub type ConvertFn = fn(&Value) -> Option<Value>;

/// A plain metadata value: what a [`DataType`]'s metadata map carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetaValue {
    Str(&'static str),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl fmt::Display for MetaValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaValue::Str(value) => write!(f, "{value:?}"),
            MetaValue::Int(value) => write!(f, "{value}"),
            MetaValue::Float(value) => write!(f, "{value}"),
            MetaValue::Bool(value) => write!(f, "{value}"),
        }
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.name, self.id)?;
        for (index, (key, value)) in self.meta.iter().enumerate() {
            write!(
                f,
                "{}{key}: {value}",
                if index == 0 { " — meta " } else { ", " }
            )?;
        }
        Ok(())
    }
}

inventory::collect! { DataType }

/// Declare a data type. This is the whole registration step: expanding it in
/// any crate that depends on `nodetool` contributes the type to the registry
/// of every binary the crate is linked into. One caveat: the linker discards
/// a crate nothing references; keep such a plugin linked with `use
/// the_plugin as _;` (see the crate docs).
///
/// ```
/// use nodetool::Value;
///
/// fn shape_area(value: &Value) -> Option<Value> {
///     None
/// }
///
/// nodetool::data_type! {
///     id: nodetool::uuid!("a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"),
///     name: "shapes/shape",
///     conversions: [ nodetool::scalars::F64 => shape_area ],  // optional
///     meta: [  // optional
///         "color" => nodetool::MetaValue::Str("#4a90d9"),
///         "shape" => nodetool::MetaValue::Str("circle"),
///     ],
/// }
/// ```
///
/// `id` is the stable uuid — [`uuid!`] parses the literal at compile time —
/// and `name` is the reference ports carry. A conversion pairs the target
/// type's id with the function performing it ([`ConvertFn`]); it may name a
/// type that is not registered yet, since targets resolve once every linked
/// crate has contributed, and a target that never shows up is reported when
/// the registry is read. Metadata is a map of plain values; a type whose
/// metadata carries both a `color` and a `shape` declares the appearance the
/// editor renders its ports and wires in, the shape one of the editor's
/// drawn set, `circle` or `square` — the editor composes the pair into the
/// listing it serves, taking the neutral for a type that does not declare
/// both.
#[macro_export]
macro_rules! data_type {
    (
        id: $id:expr,
        name: $name:literal
        $(, conversions: [ $($target:expr => $convert:expr),* $(,)? ])?
        $(, meta: [ $($key:literal => $value:expr),* $(,)? ])?
        $(,)?
    ) => {
        $crate::inventory::submit! {
            $crate::DataType {
                id: $id,
                name: $name,
                conversions: &[$($($crate::Conversion { target: $target, convert: $convert }),*)?],
                meta: &[$($(($key, $value)),*)?],
            }
        }
    };
}
