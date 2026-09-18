//! The custom-UI declarations read back from the registry: both attachment
//! points through the one `inventory` registration path — a node type's UI
//! naming its entry asset, a data type's UI naming its serialiser and entry
//! asset — and the absence facts beside them, a type declared with no UI
//! crossing contentless as the default always has.

use nodetool::Value;
use test_plugin_delta as _;
use test_plugin_epsilon as _;

#[test]
fn a_node_types_declared_ui_is_in_the_registry_with_its_entry_and_contract() {
    let node_type = nodetool::registry::node_type("epsilon/widget").expect("linked and declared");
    let ui = node_type.ui.expect("the fixture declares the node UI");
    assert_eq!(ui.contract, 1);
    assert_eq!(ui.entry, "epsilon/widget-node.js");
    assert!(
        ui.source.contains("export default"),
        "the bundle is embedded"
    );
}

#[test]
fn a_node_type_without_declared_ui_has_none() {
    let node_type = nodetool::registry::node_type("epsilon/source").expect("linked and declared");
    assert!(node_type.ui.is_none());
}

#[test]
fn a_data_types_declared_ui_carries_its_serialiser_and_entry() {
    let data_type = nodetool::registry::data_type("epsilon/tone").expect("linked and declared");
    let ui = data_type.ui.expect("the fixture declares the value UI");
    assert_eq!(ui.contract, 1);
    assert_eq!(ui.entry, "epsilon/tone-value.js");

    let value = Value::new(
        test_plugin_epsilon::TONE,
        test_plugin_epsilon::Tone { hz: 440.0 },
    );
    assert_eq!((ui.serialise)(&value).as_deref(), Some("440 Hz"));
}

#[test]
fn a_type_without_declared_ui_has_no_serialiser_to_call() {
    let scalars = [nodetool::scalars::I32, nodetool::scalars::STRING];
    for id in scalars {
        let data_type = nodetool::registry::data_type_by_id(id).expect("core ships the scalars");
        assert!(
            data_type.ui.is_none(),
            "the base scalars declare no value UI"
        );
    }
    let mark = nodetool::registry::data_type("delta/mark").expect("linked and declared");
    assert!(
        mark.ui.is_none(),
        "an undeclared custom type stays contentless"
    );
}
