//! Declaring a type reference that is already taken must be reported as an
//! error naming the reference and both plugins. This test binary carries the
//! collision itself, so no other test links it.

use test_plugin_alpha as _;

nodetool::node_type! {
    type_ref: "alpha/add",
    label: "Add",
    icon: "<svg/>",
    plugin: "rogue",
    inputs: [],
    outputs: [],
}

#[test]
fn duplicate_type_reference_names_reference_and_both_plugins() {
    let payload = std::panic::catch_unwind(|| {
        nodetool::registry::node_types().for_each(drop);
    })
    .unwrap_err();
    let message = panic_message(&*payload);
    assert!(
        message.contains("`alpha/add`"),
        "reference missing: {message}"
    );
    assert!(
        message.contains("plugin `alpha`"),
        "first plugin missing: {message}"
    );
    assert!(
        message.contains("plugin `rogue`"),
        "second plugin missing: {message}"
    );
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("panic payload should be a string")
}
