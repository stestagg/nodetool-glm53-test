//! The data type vocabulary as the registry serves it: the base scalars, a
//! plugin's custom type, and the conversions both declare.

use nodetool::registry::{data_type, data_type_by_id, data_types};
use nodetool::scalars;
use nodetool::{uuid, MetaValue};
use test_plugin_alpha as _;
use test_plugin_beta as _;

fn names() -> Vec<&'static str> {
    data_types().map(|t| t.name).collect()
}

#[test]
fn ships_the_base_scalar_set() {
    let names = names();
    for name in [
        "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "f32", "f64", "bool", "String",
    ] {
        assert!(names.contains(&name), "missing {name} in {names:?}");
    }
}

#[test]
fn registers_a_plugin_custom_type() {
    let ratio = data_type("alpha/ratio").expect("declared by test plugin alpha");
    assert_eq!(ratio.id, uuid!("d0e1f2a3-4b5c-4d6e-8f90-1a2b3c4d5e6f"));
    assert_eq!(
        ratio.meta,
        [
            ("min", MetaValue::Float(0.0)),
            ("max", MetaValue::Float(1.0)),
        ]
    );
}

#[test]
fn base_scalars_declare_the_trivial_conversions() {
    let i16 = data_type("i16").expect("base scalar");
    assert_eq!(i16.conversions.len(), 1);
    assert_eq!(i16.conversions[0].target, scalars::I32);
    let widened = (i16.conversions[0].convert)(&7i16).expect("i16 converts to i32");
    assert_eq!(widened.downcast_ref::<i32>(), Some(&7));

    let f32 = data_type("f32").expect("base scalar");
    assert_eq!(f32.conversions[0].target, scalars::F64);
    let widened = (f32.conversions[0].convert)(&1.5f32).expect("f32 converts to f64");
    assert_eq!(widened.downcast_ref::<f64>(), Some(&1.5));

    let i32 = data_type("i32").expect("base scalar");
    assert_eq!(i32.conversions[0].target, scalars::F64);
    let widened = (i32.conversions[0].convert)(&3i32).expect("i32 converts to f64");
    assert_eq!(widened.downcast_ref::<f64>(), Some(&3.0));
}

#[test]
fn scalars_without_a_trivial_conversion_declare_none() {
    for name in [
        "i8", "i64", "u8", "u16", "u32", "u64", "f64", "bool", "String",
    ] {
        let scalar = data_type(name).expect("base scalar");
        assert!(
            scalar.conversions.is_empty(),
            "{name} declares a conversion"
        );
    }
}

#[test]
fn no_conversion_targets_string() {
    let string_id = data_type("String").expect("base scalar").id;
    for declared in data_types() {
        for conversion in declared.conversions {
            assert_ne!(
                conversion.target, string_id,
                "{} declares a conversion to String",
                declared.name
            );
        }
    }
}

#[test]
fn plugin_conversion_declarations_resolve_to_registered_targets() {
    let ratio = data_type("alpha/ratio").expect("declared by test plugin alpha");
    assert_eq!(ratio.conversions.len(), 1);
    assert_eq!(ratio.conversions[0].target, scalars::F64);
    let target = data_type_by_id(ratio.conversions[0].target).expect("targets resolve on read");
    assert_eq!(target.name, "f64");
}

#[test]
fn looks_up_by_name_and_id() {
    let f64 = data_type("f64").expect("base scalar");
    assert_eq!(f64.id, scalars::F64);
    assert_eq!(
        data_type_by_id(scalars::F64).expect("base scalar").name,
        "f64"
    );
    assert!(data_type("missing/type").is_none());
}

#[test]
fn display_shows_identity_and_metadata() {
    let ratio = data_type("alpha/ratio").expect("declared by test plugin alpha");
    let listing = ratio.to_string();
    assert!(listing.contains("alpha/ratio"), "name missing: {listing}");
    assert!(
        listing.contains(&ratio.id.to_string()),
        "id missing: {listing}"
    );
    assert!(listing.contains("min: 0"), "meta missing: {listing}");
    assert!(listing.contains("max: 1"), "meta missing: {listing}");
}
