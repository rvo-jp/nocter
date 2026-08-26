//! Source and syntax projection kept beside the semantic program.
//!
//! This is the only data structure allowed to pair semantic identities with source or syntax
//! identities. Lowering boundaries may extend it when they create new identities; canonical
//! declaration, checked, and machine programs never depend on it.

mod documentation;
mod entity;
mod index;
mod names;
mod origin;

pub use documentation::{DocumentationOwner, DuplicateDocumentation};
pub use entity::SemanticEntity;
pub use index::{
    DuplicateSourceBinding, SourceAccess, SourceBinding, SourceIndex, SourceIndexBuilder,
    SourceRole,
};
pub use origin::{SourceOrigin, UnknownNodeId, UnknownTokenId};
