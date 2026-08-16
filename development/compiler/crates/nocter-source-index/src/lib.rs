//! Source and syntax projection kept beside the semantic program.
//!
//! This is the only Phase 2 data structure allowed to pair semantic identities with source or
//! syntax identities. Checked semantics and code generation do not depend on this crate.

mod entity;
mod index;
mod origin;

pub use entity::SemanticEntity;
pub use index::{
    DuplicateSourceBinding, SourceBinding, SourceIndex, SourceIndexBuilder, SourceRole,
};
pub use origin::{SourceOrigin, UnknownNodeId};
