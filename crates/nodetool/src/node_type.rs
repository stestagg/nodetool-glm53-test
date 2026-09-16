//! The node type declaration model, and the [`node_type!`] macro plugins use
//! to declare a node type.

use std::fmt;

use crate::behaviour::BehaviourFn;

/// A node type as a plugin declares it: pure data, identified by its
/// [`NodeType::type_ref`] — the key by which the registry, graph files, and
/// the compiler refer to it.
#[derive(Clone, Copy, Debug)]
pub struct NodeType {
    /// Stable reference, unique across the linked plugins.
    pub type_ref: &'static str,
    /// Default label shown in the palette and on nodes.
    pub label: &'static str,
    /// Inline SVG markup for the node.
    pub icon: &'static str,
    /// The plugin grouping this node type belongs to.
    pub plugin: &'static str,
    /// Optional sub-grouping within the plugin.
    pub sub_group: Option<&'static str>,
    /// Input ports, on the left of the node.
    pub inputs: &'static [Port],
    /// Output ports, on the right of the node.
    pub outputs: &'static [Port],
    /// Builds the behaviour each instance runs — the authoring API's half of
    /// the declaration. A type declared without one is a descriptor alone:
    /// the compiler accepts it, but nothing runs it.
    pub behaviour: Option<BehaviourFn>,
}

/// One input or output port of a [`NodeType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port {
    pub name: &'static str,
    /// The type references this port carries — one or more, as declared.
    pub type_refs: &'static [&'static str],
}

inventory::collect! { NodeType }

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}] — plugin {}",
            self.label, self.type_ref, self.plugin
        )?;
        if let Some(sub_group) = self.sub_group {
            write!(f, ", sub-group {sub_group}")?;
        }
        for port in self.inputs {
            write!(f, "\n  in  {}: {}", port.name, port.type_refs.join(", "))?;
        }
        for port in self.outputs {
            write!(f, "\n  out {}: {}", port.name, port.type_refs.join(", "))?;
        }
        Ok(())
    }
}

/// Declare a node type. This is the whole registration step: expanding it in
/// any crate that depends on `nodetool` contributes the node type to the
/// registry of every binary the crate is linked into. One caveat: the linker
/// discards a crate nothing references; keep such a plugin linked with `use
/// the_plugin as _;` (see the crate docs).
///
/// ```rust
/// fn circle_behaviour() -> Box<dyn nodetool::behaviour::Behaviour> {
///     unimplemented!("the behaviour is the authoring API's business")
/// }
///
/// nodetool::node_type! {
///     type_ref: "shapes/circle",
///     label: "Circle",
///     icon: r#"<svg ...>...</svg>"#,
///     plugin: "shapes",
///     sub_group: "2d",               // optional
///     behaviour: circle_behaviour,   // optional; see nodetool::behaviour
///     inputs:  [ radius: ["i32", "f64"] ],
///     outputs: [ shape: "shapes/shape" ],
/// }
/// ```
///
/// A port carries one or more declared type references; write them as a single
/// literal or a bracketed list. The optional `behaviour` arm names a
/// `fn() -> Box<dyn nodetool::behaviour::Behaviour>` that builds the behaviour
/// each instance runs — the stream semantics it programs against live in
/// [`nodetool::behaviour`](crate::behaviour).
#[macro_export]
macro_rules! node_type {
    (
        type_ref: $type_ref:literal,
        label: $label:literal,
        icon: $icon:literal,
        plugin: $plugin:literal
        $(, sub_group: $sub_group:literal)?
        $(, behaviour: $behaviour:path)?
        ,
        inputs: [ $($input_name:ident : $input_types:tt),* $(,)? ]
        ,
        outputs: [ $($output_name:ident : $output_types:tt),* $(,)? ]
        $(,)?
    ) => {
        $crate::inventory::submit! {
            $crate::NodeType {
                type_ref: $type_ref,
                label: $label,
                icon: $icon,
                plugin: $plugin,
                sub_group: $crate::node_type!(@sub_group $($sub_group)?),
                behaviour: $crate::node_type!(@behaviour $($behaviour)?),
                inputs: $crate::node_type!(@ports $($input_name : $input_types),*),
                outputs: $crate::node_type!(@ports $($output_name : $output_types),*),
            }
        }
    };
    (@sub_group $sub_group:literal) => { Some($sub_group) };
    (@sub_group) => { None };
    (@behaviour $behaviour:path) => { Some($behaviour) };
    (@behaviour) => { None };
    (@ports $($name:ident : $types:tt),*) => {
        &[$(
            $crate::Port {
                name: stringify!($name),
                type_refs: $crate::__port_type_refs!($types),
            }
        ),*]
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __port_type_refs {
    ([$($type_ref:literal),+ $(,)?]) => { &[$($type_ref),*] };
    ($type_ref:literal) => { &[$type_ref] };
}
