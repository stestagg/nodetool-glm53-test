//! The base scalar set core ships, declared through the same [`data_type!`]
//! mechanism a plugin uses — they are an instance of the general capability,
//! not a core special case. The trivial conversions the base set declares are
//! the named widenings: `i16`→`i32`, `f32`→`f64`, and the slightly looser
//! `i32`→`f64`; widening that set later is an ordinary declaration. No
//! conversion to `String` is declared anywhere: turning values into strings
//! is the explicit Format node's job.
//!
//! Each scalar also declares its appearance — a colour and a port shape,
//! the same metadata a plugin's custom types carry. Declaring is all core
//! does with them: the editor's listing composes them into the fact it
//! serves the browser, and nothing in core switches on either.

use uuid::Uuid;

use crate::{MetaValue, Value};

/// The base scalars' fixed ids, for conversions that name them as targets.
pub const I8: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000001");
pub const I16: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000002");
pub const I32: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000003");
pub const I64: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000004");
pub const U8: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000005");
pub const U16: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000006");
pub const U32: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000007");
pub const U64: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000008");
pub const F32: Uuid = uuid::uuid!("00000000-0000-0000-0000-000000000009");
pub const F64: Uuid = uuid::uuid!("00000000-0000-0000-0000-00000000000a");
pub const BOOL: Uuid = uuid::uuid!("00000000-0000-0000-0000-00000000000b");
pub const STRING: Uuid = uuid::uuid!("00000000-0000-0000-0000-00000000000c");

/// Whether a data type id names one of the base scalars core ships. The
/// one fact the editor's scalar-field rule reads about a type; the base
/// set grows by declaring here, and the fact follows.
pub fn is_base_scalar(id: Uuid) -> bool {
    [I8, I16, I32, I64, U8, U16, U32, U64, F32, F64, BOOL, STRING].contains(&id)
}

/// A base scalar's plain text form — the same reading the canvas holds of
/// a scalar emission and the headless timeline prints of one. None for
/// any other value: a plugin custom type's rendering is its plugin's
/// business, so core invents nothing to stand in for it.
pub fn scalar_text(value: &Value) -> Option<String> {
    if !is_base_scalar(value.type_id()) {
        return None;
    }
    macro_rules! scalars {
        ($($ty:ty),* $(,)?) => {$(
            if let Some(text) = value.get::<$ty>().map(ToString::to_string) {
                return Some(text);
            }
        )*};
    }
    scalars!(String, bool, i8, i16, i32, i64, u8, u16, u32, u64, f32, f64);
    None
}

// The appearance declarations: a hue per family from the editor's
// Blueprint-adjacent palette, the numeric families square and the rest
// round — the shape distinguishing beside the colour, never colour alone.
const NUMERIC_COLOR: &str = "#2d72d2";
const FLOAT_COLOR: &str = "#9d3f9d";
const FLAG_COLOR: &str = "#d1820c";
const TEXT_COLOR: &str = "#238551";
const SQUARE: &str = "square";
const ROUND: &str = "circle";

crate::data_type! { id: I8, name: "i8", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: I16, name: "i16", conversions: [ I32 => i16_to_i32 ], meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: I32, name: "i32", conversions: [ F64 => i32_to_f64 ], meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: I64, name: "i64", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: U8, name: "u8", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: U16, name: "u16", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: U32, name: "u32", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: U64, name: "u64", meta: [ "color" => MetaValue::Str(NUMERIC_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: F32, name: "f32", conversions: [ F64 => f32_to_f64 ], meta: [ "color" => MetaValue::Str(FLOAT_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: F64, name: "f64", meta: [ "color" => MetaValue::Str(FLOAT_COLOR), "shape" => MetaValue::Str(SQUARE) ] }
crate::data_type! { id: BOOL, name: "bool", meta: [ "color" => MetaValue::Str(FLAG_COLOR), "shape" => MetaValue::Str(ROUND) ] }
crate::data_type! { id: STRING, name: "String", meta: [ "color" => MetaValue::Str(TEXT_COLOR), "shape" => MetaValue::Str(ROUND) ] }

fn i16_to_i32(value: &Value) -> Option<Value> {
    Some(Value::new(I32, *value.get::<i16>()? as i32))
}

fn f32_to_f64(value: &Value) -> Option<Value> {
    Some(Value::new(F64, *value.get::<f32>()? as f64))
}

fn i32_to_f64(value: &Value) -> Option<Value> {
    Some(Value::new(F64, *value.get::<i32>()? as f64))
}
