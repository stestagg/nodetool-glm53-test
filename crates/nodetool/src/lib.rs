//! Nodetool core: the abstract node-type model, the shared vocabulary of data
//! types, and the registry that gathers node type and data type declarations
//! from every crate linked into the binary.
//!
//! Core knows what a node type *is* — a stable type reference, a label, an
//! icon, a grouping, typed ports — and what a data type *is* — a stable uuid,
//! a name, declared conversions, metadata — but contains no node types and no
//! custom data types of its own. It does ship the base scalar set
//! ([`scalars`]). Node libraries are ordinary crates that depend only on
//! `nodetool`, declare their node types with [`node_type!`] and their custom
//! data types with [`data_type!`], and are linked into a binary. That link is
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
pub use uuid::{uuid, Uuid};

mod data_type;
mod node_type;
pub mod registry;
pub mod scalars;

pub use data_type::{Conversion, ConvertFn, DataType, MetaValue};
pub use node_type::{NodeType, Port};
