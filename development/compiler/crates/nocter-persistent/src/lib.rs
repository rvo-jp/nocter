#![allow(clippy::disallowed_types)]

//! Dependency-free persistent collections used by compiler construction authorities.
//!
//! These collections know nothing about Nocter semantics. Semantic crates keep them behind their
//! immutable authority and transaction contracts; downstream compiler products never expose their
//! roots or nodes.

mod map;
mod vector;

pub use map::PersistentMap;
pub use vector::{PersistentVector, PersistentVectorIter};
