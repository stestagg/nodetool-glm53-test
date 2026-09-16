//! Registering a data type whose name is already taken must be reported as an
//! error, not silently replaced or ignored. This test binary carries the
//! collision itself, so no other test links it.

mod common;

nodetool::data_type! {
    id: nodetool::uuid!("1f2e3d4c-5b6a-4978-8a9b-cd0e1f2a3b4c"),
    name: "i32",
}

#[test]
fn duplicate_name_names_the_name_and_both_declarations() {
    common::assert_panic_message(
        || {
            nodetool::registry::data_types().for_each(drop);
        },
        &[
            "duplicate data type name `i32`",
            "00000000-0000-0000-0000-000000000003",
            "1f2e3d4c-5b6a-4978-8a9b-cd0e1f2a3b4c",
        ],
    );
}

#[test]
fn lookup_reports_a_taken_name_too() {
    common::assert_panic_message(
        || {
            let _ = nodetool::registry::data_type("i32");
        },
        &["duplicate data type name `i32`"],
    );
}
