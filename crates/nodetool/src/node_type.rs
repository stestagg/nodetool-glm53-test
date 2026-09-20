//! The node type declaration model, and the [`node_type!`] macro plugins use
//! to declare a node type.

use std::fmt;

use crate::behaviour::BehaviourFn;
use crate::compile::CompiledNode;

/// A node type's custom UI, as the declaring plugin embeds it: one bundle,
/// carried in the crate at build time and served by the editor's HTTP
/// server under a per-plugin path, that replaces the default node rendering
/// for the type. The browser learns the entry from the listing the server
/// serves — never a hardcoded path — and loads it against the component
/// contract version the bundle names.
#[derive(Clone, Copy, Debug)]
pub struct NodeUi {
    /// The component contract version the bundle was built against. The
    /// editor offers a versioned contract; a bundle naming one it does not
    /// speak is known before it loads, and the node falls back to the
    /// default rendering.
    pub contract: u64,
    /// The bundle's entry asset, plugin-namespaced — `shapes/stage-node.js`
    /// is served at `/plugins/shapes/stage-node.js`.
    pub entry: &'static str,
    /// The bundle's text, embedded at build time.
    pub source: &'static str,
}

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
    /// Optional custom node UI: the bundle that renders the type's body
    /// between the editor's title bar and ports. A type declared without
    /// one renders by the default node class.
    pub ui: Option<NodeUi>,
    /// Input ports, on the left of the node.
    pub inputs: &'static [Port],
    /// Output ports, on the right of the node.
    pub outputs: &'static [Port],
    /// The settings each instance chooses from a fixed set of options —
    /// rendered in the middle of the node, between the ports it sits
    /// among. Declaring none is the ordinary case.
    pub choices: &'static [Choice],
    /// Builds the behaviour each instance runs — the authoring API's half of
    /// the declaration. A type declared without one is a descriptor alone:
    /// the compiler accepts it, but nothing runs it.
    pub behaviour: Option<BehaviourFn>,
    /// Validates the compiled instance at compile time, when the node type
    /// can say something about it the type rules cannot — a zero step, an
    /// inverted range, an input its behaviour gates on that nothing
    /// carries. The compiler consults it after the parameters and any port
    /// families resolved; every returned line is a compile error. A type
    /// declared without one has nothing to add.
    pub check_parameters: Option<ParameterCheck>,
}

/// A named setting whose value is one of a fixed set of options: the node
/// type declares the name and the options, each instance holds its chosen
/// option as an ordinary parameter value under that name, and the compiler
/// refuses an instance whose choice is absent or outside the set — so a
/// behaviour reads the setting through [`CompiledNode::choice`] knowing
/// compile guaranteed it ([`crate::compile`]). A choice is not a port:
/// nothing connects to it, no value flows into it, and it takes no part in
/// the hang gate. What a choice *means* is the declaring plugin's business
/// — core knows only the name and the options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    pub name: &'static str,
    /// The options the setting offers, in the order the editor lists them.
    pub options: &'static [&'static str],
}

/// One input or output port of a [`NodeType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Port {
    pub name: &'static str,
    /// The type references this port carries — one or more, as declared.
    pub type_refs: &'static [&'static str],
    /// The port family this port belongs to, if any: on one node type, the
    /// ports of one family resolve to a single concrete type at compile
    /// time, drawn from the members the ports declare. A generic numeric
    /// node — one descriptor, any scalar — is the shape this serves.
    pub family: Option<&'static str>,
}

/// Validates one compiled node instance, from the plugin that declared the
/// node type. The compiler consults it after the parameters and any port
/// families resolved; each returned line is a compile error the compiler
/// reports naming the node.
pub type ParameterCheck = fn(&CompiledNode) -> Vec<String>;

/// The compile-time form of an input a behaviour gates on but nothing
/// carries: neither a connection feeds it nor a parameter holds it, so the
/// gate waits on a stream that is empty and stays empty — the run would
/// stall there without end, with nothing to read and no error to show. A
/// [`ParameterCheck`] hands this the names its behaviour gates on; each one
/// carried by neither is a compile error naming it.
pub fn carried_check(compiled: &CompiledNode, names: &[&str]) -> Vec<String> {
    names
        .iter()
        .filter(|name| !compiled.parameters.contains_key(*name) && !compiled.fed.contains(*name))
        .map(|name| {
            format!("input `{name}` holds no parameter value and no connection feeds it — the node would never fire")
        })
        .collect()
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
        for choice in self.choices {
            write!(
                f,
                "\n  choice {}: {}",
                choice.name,
                choice.options.join(", ")
            )?;
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
/// fn circle_behaviour(_compiled: &nodetool::compile::CompiledNode)
///     -> Box<dyn nodetool::behaviour::Behaviour>
/// {
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
///     ui: nodetool::NodeUi {         // optional; custom node UI
///         contract: 1,
///         entry: "shapes/circle-node.js",
///         source: r#"export default function CircleNode() { ... }"#,
///     },
///     choices: [ fill: ["solid", "outline"] ],   // optional
///     inputs:  [ radius: ["i32", "f64"] ],
///     outputs: [ shape: "shapes/shape" ],
/// }
/// ```
///
/// The optional `ui` arm names a [`NodeUi`] — the bundle rendering the
/// type's body in the editor, embedded at build time and served under the
/// plugin's asset path; see `crates/nodetool/ui/src/pluginui.jsx` for the
/// component contract the bundle is built against.
///
/// A port carries one or more declared type references; write them as a single
/// literal or a bracketed list. A port may instead declare a *port family*:
/// `name: ["num", [members...]]` (or `name: ["num", MEMBERS]` for a members
/// const) — the members as its type references, and the family name marking
/// the ports that must resolve to one concrete type at compile time (see
/// [`Port::family`]). The optional `behaviour` arm
/// names a `fn(&CompiledNode) -> Box<dyn nodetool::behaviour::Behaviour>`
/// that builds the behaviour each instance runs — the compiled node carries
/// the instance's resolved port families, so a family-declared node stamps
/// its behaviour for the resolved type — and the stream semantics it
/// programs against live in [`nodetool::behaviour`](crate::behaviour). The
/// optional `check_parameters` arm names a [`ParameterCheck`] the compiler
/// consults once the instance's parameters and families resolved.
///
/// The optional `choices` arm declares the type's [`Choice`] settings —
/// `name: [option, ...]` apiece. A choice is not a port: the instance holds
/// its chosen option among its parameters, under the choice's name, and the
/// behaviour reads it back with
/// [`CompiledNode::choice`](crate::compile::CompiledNode::choice).
#[macro_export]
macro_rules! node_type {
    (
        type_ref: $type_ref:literal,
        label: $label:literal,
        icon: $icon:literal,
        plugin: $plugin:literal
        $(, sub_group: $sub_group:literal)?
        $(, behaviour: $behaviour:path)?
        $(, check_parameters: $check:path)?
        $(, ui: $ui:expr)?
        $(, choices: [ $($choice_name:ident : [$($option:literal),+ $(,)?]),* $(,)? ])?
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
                check_parameters: $crate::node_type!(@check $($check)?),
                ui: $crate::node_type!(@ui $($ui)?),
                inputs: &[$($crate::node_type!(@port $input_name : $input_types)),*],
                outputs: &[$($crate::node_type!(@port $output_name : $output_types)),*],
                choices: &[$($($crate::Choice {
                    name: stringify!($choice_name),
                    options: &[$($option),*],
                }),*)?],
            }
        }
    };
    (@sub_group $sub_group:literal) => { Some($sub_group) };
    (@sub_group) => { None };
    (@behaviour $behaviour:path) => { Some($behaviour) };
    (@behaviour) => { None };
    (@check $check:path) => { Some($check) };
    (@check) => { None };
    (@ui $ui:expr) => { Some($ui) };
    (@ui) => { None };
    (@port $name:ident : [$family:literal, [$($type:literal),+ $(,)?]]) => {
        $crate::Port {
            name: stringify!($name),
            type_refs: &[$($type),*],
            family: Some($family),
        }
    };
    (@port $name:ident : [$family:literal, $members:path]) => {
        $crate::Port {
            name: stringify!($name),
            type_refs: $members,
            family: Some($family),
        }
    };
    (@port $name:ident : [$($type:literal),+ $(,)?]) => {
        $crate::Port {
            name: stringify!($name),
            type_refs: &[$($type),*],
            family: None,
        }
    };
    (@port $name:ident : $type:literal) => {
        $crate::Port {
            name: stringify!($name),
            type_refs: &[$type],
            family: None,
        }
    };
}
