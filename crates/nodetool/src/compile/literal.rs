//! How a parameter literal becomes a runtime value: the reading a compile
//! performs when a definition fixes an input's value as text. The
//! literal's YAML scalar kind stands in as the upstream side under the
//! same connection rules a wired input follows — exact kind first, then
//! the first declared conversion that bridges — so a hand-written file's
//! `3` and the editor's committed `3` compile to the same value, by the
//! same rules, resolved at compile time.

use crate::graph::ParameterValue;
use crate::registry::Registry;
use crate::value::Value;
use crate::{ConvertFn, DataType};

/// Resolve a parameter literal against an input port's declared union, under
/// the connection rules: the literal's YAML scalar kind stands in as the
/// upstream side. Returns the resolved data type and the runtime value the
/// engine feeds — converted here, at compile time, where a declared
/// conversion bridged.
pub(super) fn resolve_literal(
    literal: &ParameterValue,
    declared: &[&str],
    registry: &Registry,
) -> Option<(&'static DataType, Value)> {
    let kind = literal_kind(literal);
    for &name in declared {
        let Some(data_type) = registry.data_type(name) else {
            continue;
        };
        if scalar_kind(data_type.name) == Some(kind) {
            if let Some(value) = materialize(literal, data_type) {
                return Some((data_type, value));
            }
        }
    }
    for &name in declared {
        let Some(data_type) = registry.data_type(name) else {
            continue;
        };
        for source in kind_names(kind) {
            let Some(source_type) = registry.data_type(source) else {
                continue;
            };
            for conversion in source_type.conversions {
                if conversion.target == data_type.id {
                    if let Some(value) = convert_literal(literal, source_type, conversion.convert) {
                        return Some((data_type, value));
                    }
                }
            }
        }
    }
    None
}

/// The four YAML scalar kinds a parameter literal can carry, and the scalar
/// names standing in for each kind: the literal's kind is its "upstream
/// side" under the connection rules, and the lists fix the order conversion
/// candidates are tried in.
const KIND_NAMES: &[(ScalarKind, &[&str])] = &[
    (
        ScalarKind::Integer,
        &["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"],
    ),
    (ScalarKind::Float, &["f32", "f64"]),
    (ScalarKind::Bool, &["bool"]),
    (ScalarKind::Str, &["String"]),
];

#[derive(Clone, Copy, PartialEq)]
enum ScalarKind {
    Integer,
    Float,
    Bool,
    Str,
}

fn literal_kind(literal: &ParameterValue) -> ScalarKind {
    match literal {
        ParameterValue::Bool(_) => ScalarKind::Bool,
        ParameterValue::Int(_) => ScalarKind::Integer,
        ParameterValue::Float(_) => ScalarKind::Float,
        ParameterValue::Str(_) => ScalarKind::Str,
    }
}

fn scalar_kind(name: &str) -> Option<ScalarKind> {
    KIND_NAMES
        .iter()
        .find(|(_, names)| names.contains(&name))
        .map(|(kind, _)| *kind)
}

fn kind_names(kind: ScalarKind) -> &'static [&'static str] {
    KIND_NAMES
        .iter()
        .find(|(candidate, _)| *candidate == kind)
        .map(|(_, names)| *names)
        .expect("every ScalarKind is listed in KIND_NAMES")
}

/// Present a literal as a runtime value of the data type `source` names, so
/// exact matches and declared conversion functions read a concrete scalar.
/// An integer outside the source's range yields None: the candidate simply
/// does not bridge. A float wider than f32's range yields None too; one that
/// fits is narrowed.
fn materialize(literal: &ParameterValue, source: &DataType) -> Option<Value> {
    match (literal, source.name) {
        (ParameterValue::Bool(value), "bool") => Some(Value::new(source.id, *value)),
        (ParameterValue::Int(value), "i64") => Some(Value::new(source.id, *value)),
        (ParameterValue::Int(value), "i32") => {
            Some(Value::new(source.id, i32::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "i16") => {
            Some(Value::new(source.id, i16::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "i8") => {
            Some(Value::new(source.id, i8::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u64") => {
            Some(Value::new(source.id, u64::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u32") => {
            Some(Value::new(source.id, u32::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u16") => {
            Some(Value::new(source.id, u16::try_from(*value).ok()?))
        }
        (ParameterValue::Int(value), "u8") => {
            Some(Value::new(source.id, u8::try_from(*value).ok()?))
        }
        (ParameterValue::Float(value), "f64") => Some(Value::new(source.id, *value)),
        (ParameterValue::Float(value), "f32") => {
            let narrowed = *value as f32;
            if !narrowed.is_finite() {
                return None;
            }
            Some(Value::new(source.id, narrowed))
        }
        (ParameterValue::Str(value), "String") => Some(Value::new(source.id, value.clone())),
        _ => None,
    }
}

/// Apply a declared conversion to a literal, through the runtime value both
/// sides are written against.
fn convert_literal(
    literal: &ParameterValue,
    source: &DataType,
    convert: ConvertFn,
) -> Option<Value> {
    convert(&materialize(literal, source)?)
}
