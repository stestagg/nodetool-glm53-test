//! Nodetool core: the abstract node-type model and the registry that gathers
//! node type declarations from every plugin crate linked into the binary.
//!
//! Core knows what a node type *is* — a stable type reference, a label, an
//! icon, a grouping, typed ports — and contains no node types of its own. Node
//! libraries are ordinary crates that depend only on `nodetool`, declare their
//! node types with [`node_type!`], and are linked into a binary. That link is
//! the whole integration step: `inventory` collects the declarations at
//! static-initialisation time, and [`registry`] serves them aggregated across
//! plugins.
//!
//! One wrinkle in "linking is everything": the linker discards an rlib
//! that nothing references, and its declarations go with it. A binary or test
//! that never otherwise names a plugin crate keeps it linked with one anchor
//! line per plugin crate:
//!
//! ```text
//! use my_plugin as _;
//! ```

pub use inventory;

mod node_type;
pub mod registry;

pub use node_type::{NodeType, Port};
