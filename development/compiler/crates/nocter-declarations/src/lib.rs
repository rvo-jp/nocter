//! Immutable, syntax-independent declaration graph.
//!
//! This crate depends only on [`nocter_model`]. It cannot contain source files, byte ranges,
//! syntax nodes, or rendered type spellings. Syntax lowering constructs a [`DeclarationProgram`]
//! and a separate source index; semantic stages consume this program without importing the
//! lowering or syntax crates.

mod path;
mod program;
mod visibility;

pub use path::ModulePath;
pub use program::{
    DeclarationProgram, DeclarationProgramBuilder, DeclarationSite, Module, Package,
    ProgramBuildError,
};
pub use visibility::Visibility;
