//! `inventory` collects declarations in unspecified order, so a conversion
//! may be declared before its target registers — and must resolve once every
//! crate has contributed. A target that never registers is reported, not
//! silently dropped. This test binary carries the dangling declaration
//! itself, so no other test links it.

mod common;

const NOWHERE: nodetool::Uuid = nodetool::uuid!("ffffffff-ffff-4fff-8fff-ffffffffffff");

fn nowhere(_: &dyn std::any::Any) -> Option<Box<dyn std::any::Any>> {
    None
}

nodetool::data_type! {
    id: nodetool::uuid!("0e0e0e0e-0e0e-4e0e-8e0e-0e0e0e0e0e0e"),
    name: "rogue/lost",
    conversions: [ NOWHERE => nowhere ],
}

#[test]
fn a_target_that_never_registers_is_reported() {
    common::assert_panic_message(
        || {
            nodetool::registry::data_types().for_each(drop);
        },
        &[
            "`rogue/lost`",
            "conversion to id `ffffffff-ffff-4fff-8fff-ffffffffffff`",
            "no type with that id is registered",
        ],
    );
}
