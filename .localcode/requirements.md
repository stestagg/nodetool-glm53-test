# Requirements

## Product

- **REQ-1**: Nodetool is a rust crate that can be used to show/edit/manage/execute asynchronous DAG style execution graphs as a collection of nodes and collections.
- **REQ-2**: Nodetool is a very elegant, simple, but powerful tool for working with this class of asynchronous DAG execution, finding the right balance between flexibility, extensibility, and implementation simplicity.

## Core crate

- **REQ-3**: The core crate just has the base functionality, no Node implementations (beyond maybe some trivial utility nodes).
- **REQ-4**: The core crate has a server / html view pair for editing a graph.
- **REQ-5**: The core crate has logic for defining node types.
- **REQ-6**: The core crate has the ability to execute a node graph in fully streaming mode.
- **REQ-7**: The core should be able to load a graph definition and run it without the UI.

## Plugins and extensibility

- **REQ-8**: Actual node type implementations are based on a plugin architecture using `inventory`, so that users of nodetool can add their own node libraries.
- **REQ-9**: The system is designed to be highly extensible, allowing developers to add new node types, custom data types, and specialized UI components through plugins, without modifying the core crate.
- **REQ-10**: Plugin nodes are organised by plugin, and optionally sub-groupings; if a plugin provides large numbers of node types, they can be sub-grouped.

## Graph definition

- **REQ-11**: The definition of a graph is a static file (yaml?).
- **REQ-12**: Visual/graphical information (positioning etc.) is included as metadata against a node.

## Engine

- **REQ-13**: The engine (actual managing and running of nodes) is built with an optional Events generic/dyn ref that has callbacks for different events, so users can fully inspect what the engine is doing.
- **REQ-14**: The Events callbacks are used if a graph execution is started in a visual way with the editor, so nodes update as execution happens.
- **REQ-15**: The thing that runs is probably a compiled graph object based on the graph definition.

## Execution semantics

- **REQ-16**: All nodes, when running, propagate values as they become available, asynchronously.
- **REQ-17**: All connectors represent async (iterator) streams; constant values can appear constant etc.
- **REQ-18**: There is the concept of 'a new value arrived' vs 'here's the current value'.
- **REQ-19**: There is the concept of 'complete' vs 'new data may still arrive'.
- **REQ-20**: As this is modelling a data flow DAG, node outputs can connect to multiple downstream inputs.
- **REQ-21**: Inputs can only have one upstream connection.
- **REQ-22**: The execution finishes when all nodes are 'complete'.
- **REQ-23**: Most nodes become complete after they have finished processing their last input value for their inputs, all of which are now complete.

## Runtime graph modification

- **REQ-24**: For performance/simplicity, when a graph is running, it should not be able to be modified.
- **REQ-25**: In the UI, while running, node modifications should be disabled.
- **REQ-26**: Stopping and re-starting should trigger a recompile, and this should be fast/efficient.

## Types

- **REQ-27**: All inputs/outputs are typed; node types define the type of each input/output, but this can be a list/union of types.
- **REQ-28**: The type is resolved before the graph is built for execution by looking at the connection types.
- **REQ-29**: The core crate defines a base set of scalar types aligned/matching the rust types, such as `i32`, `f64`, `bool`, and `String`, etc.
- **REQ-30**: A plugin can define additional custom types as needed.
- **REQ-31**: Custom types have a uuid, a(n optional) type conversion matrix function to adapt to other types transparently, and any additional metadata required for the type.
- **REQ-32**: Custom type details are opaque to the actual core/graph/editor; unless, for example, a plugin customizes a node to display the custom type in run mode, but then the plugin has to handle serialization, deserialization, rendering etc.
- **REQ-33**: The core does not know anything about custom data types, except the top-level metadata (name, id etc.).
- **REQ-34**: A plugin should be able to define ser/de for the data (where possible) and access that data in a custom node UI section if needed for display.
- **REQ-35**: Trivial type conversions should 'just work', i.e. i16 > i32, f32 > f64, and even 'largely trivial' ones like i32 > f64.
- **REQ-36**: Probably not automatic conversion to string (see Format node).

## Nodes

- **REQ-37**: A node is like a blender node: inputs on the left, outputs on the right, other controls in the middle.
- **REQ-38**: The default node class automates this from the node type definition (types, names and numbers of inputs/outputs, helper inputs for values that can be provided as scalar without needing additional scalar value nodes).
- **REQ-39**: Plugins can customize the node UI if required.
- **REQ-40**: Nodes by default get a plugin-defined title/label, but users can edit/override this to any name.
- **REQ-41**: Node instances are internally identified by uuid.
- **REQ-42**: Nodes have icons (svg), name, uuid, input/output port definitions, and optional custom UI components for enhanced interaction.
- **REQ-43**: Nodes have backend functionality for running the node logic and status reporting.

## Groups and subgraphs

- **REQ-44**: A node that is a 'group' or 'container' node for a subgraph, allowing complex logic to be packaged up, should be designed in to the solution — including whether the UI supports it and whether the engine works with nested execution — even if it is not implemented yet.

## UI

- **REQ-45**: The UI is a react-flow style (could literally use react flow) graphical editor, connected to a small server backend over websocket.
- **REQ-46**: The UI shows a palette of available node types on the side, allowing users to drag and drop nodes out of the sidebar/palette onto the canvas to construct a graph.
- **REQ-47**: In a node graph, space is at a premium; the node editor exposes just the required information, while keeping the nodes as compact as possible.
- **REQ-48**: Colour, icons, and shapes can be useful for things like types etc.
- **REQ-49**: If in doubt, use the blender UI as a reference for what information should go where.
- **REQ-50**: Default to a light, blueprint-themed color scheme/look-and-feel.

## UI interactions

- **REQ-51**: Click-drag on background for panning.
- **REQ-52**: Scroll zoom.
- **REQ-53**: Nodes can be selected, moved using their title area (or anywhere if multi-selected).
- **REQ-54**: Nodes are connected by dragging from output to input.
- **REQ-55**: Shift-drag for multi-selection rectangle.
- **REQ-56**: Node editing is a right-hand sidebar for the selected node (or common fields across multi-selected nodes?), including global node attributes (name etc.), and then custom/per input-output fields below.
- **REQ-57**: By default, all node values should allow an input to be connected.
- **REQ-58**: The editor pane shows an input field if a scalar is possible, with the input being replaced with a connected/type indicator when a connection is made.
- **REQ-59**: If interactive mode is enabled, have a start/stop button.
- **REQ-60**: Consider when to enforce graph errors vs allowing editing despite (or to fix) them (ideally, warn early, enforce as late as possible).

## Example: nodetool-fizzbuzz

- **REQ-61**: The project should have a crate: nodetool-fizzbuzz.
- **REQ-62**: nodetool-fizzbuzz defines a Counter node that iterates from `start` to `stop` by adding `step` each time, supporting all numeric types, with basic range etc checking.
- **REQ-63**: nodetool-fizzbuzz defines a Condition operator node that takes two inputs and applies a specified boolean operation, such as equality, inequality, greater than, less than, etc., supporting all numeric types -> bool.
- **REQ-64**: An 'if' node routes values to outputs based on a Boolean input (this can be part of the core node library).
- **REQ-65**: A Format node converts input values to string using an optional format string (otherwise standard toString() style behavior is used) (this can be part of the core node library).
- **REQ-66**: An Output node accepts string inputs and collects them in a vec.
- **REQ-67**: nodetool-fizzbuzz is a binary crate.
- **REQ-68**: In 'headless' mode, it just prints the output values as they arrive, based on a graph file being provided.
- **REQ-69**: Or it launches the UI, loads the graph file, allows editing, and then runs the graph, still printing the output to the console.

## General principles

- **REQ-70**: Clean simple design.
- **REQ-71**: Errors get reported, and typically get raised up; never catch an error and just 'return None' or silently ignore unless it's very clearly requested or the right thing to do.
- **REQ-72**: Don't over-validate, or do unnecessary checks, especially ones that might add extra failure modes, unless they are truly needed for correct operation; let a failure occur naturally where possible.
- **REQ-73**: The core should never ever switch behaviour based on a specific node type or name/id/detail of a node that isn't generic; at every change, the question is: does this belong in core as a general, abstract capability, or is this a plugin-specific concern?

## Development approach

- **REQ-74**: The work should be implemented across multiple stories to keep the cognitive scope of each change (the reasoning required to implement the core engine correctly should not be under-estimated).

## Definition of done

- **REQ-75**: The project can be considered complete when the above requirements have been implemented such that the example nodetool-fizzbuzz crate works as intended.
