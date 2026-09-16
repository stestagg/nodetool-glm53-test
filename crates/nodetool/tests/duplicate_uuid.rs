//! Registering a data type whose uuid is already taken must be reported as an
//! error, not silently replaced or ignored. This test binary carries the
//! collision itself, so no other test links it.

mod common;

nodetool::data_type! {
    id: nodetool::uuid!("00000000-0000-0000-0000-00000000000a"),
    name: "rogue/f64",
}

#[test]
fn duplicate_id_names_the_id_and_both_declarations() {
    common::assert_panic_message(
        || {
            nodetool::registry::data_types().for_each(drop);
        },
        &[
            "duplicate data type id `00000000-0000-0000-0000-00000000000a`",
            "`f64`",
            "`rogue/f64`",
        ],
    );
}

#[test]
fn lookup_reports_a_taken_id_too() {
    common::assert_panic_message(
        || {
            let id = nodetool::uuid!("00000000-0000-0000-0000-00000000000a");
            let _ = nodetool::registry::data_type_by_id(id);
        },
        &["duplicate data type id `00000000-0000-0000-0000-00000000000a`"],
    );
}
