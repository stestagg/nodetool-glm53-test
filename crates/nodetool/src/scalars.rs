//! The base scalar set core ships, declared through the same [`data_type!`]
//! mechanism a plugin uses — they are an instance of the general capability,
//! not a core special case. The trivial conversions the base set declares are
//! the named widenings: `i16`→`i32`, `f32`→`f64`, and the slightly looser
//! `i32`→`f64`; widening that set later is an ordinary declaration. No
//! conversion to `String` is declared anywhere: turning values into strings
//! is the explicit Format node's job.

use std::any::Any;

use uuid::Uuid;

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

crate::data_type! { id: I8, name: "i8" }
crate::data_type! { id: I16, name: "i16", conversions: [ I32 => i16_to_i32 ] }
crate::data_type! { id: I32, name: "i32", conversions: [ F64 => i32_to_f64 ] }
crate::data_type! { id: I64, name: "i64" }
crate::data_type! { id: U8, name: "u8" }
crate::data_type! { id: U16, name: "u16" }
crate::data_type! { id: U32, name: "u32" }
crate::data_type! { id: U64, name: "u64" }
crate::data_type! { id: F32, name: "f32", conversions: [ F64 => f32_to_f64 ] }
crate::data_type! { id: F64, name: "f64" }
crate::data_type! { id: BOOL, name: "bool" }
crate::data_type! { id: STRING, name: "String" }

fn i16_to_i32(value: &dyn Any) -> Option<Box<dyn Any>> {
    Some(Box::new(*value.downcast_ref::<i16>()? as i32))
}

fn f32_to_f64(value: &dyn Any) -> Option<Box<dyn Any>> {
    Some(Box::new(*value.downcast_ref::<f32>()? as f64))
}

fn i32_to_f64(value: &dyn Any) -> Option<Box<dyn Any>> {
    Some(Box::new(*value.downcast_ref::<i32>()? as f64))
}
