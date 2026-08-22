//! Source-independent contracts shared across target closure, MIR, and machine lowering.
//!
//! This crate owns identities and immutable representation facts, not the implementation that
//! discovers, validates, lowers, lays out, or executes them. It depends only on semantic model
//! identities so no backend consumer gains access to checked or target-program storage.

mod primitive;
mod representation;
mod target;

pub use primitive::{PrimitiveBinding, PrimitiveBindingError, PrimitiveRegistry, PrimitiveRole};
pub use representation::{
    RuntimeFieldRepresentation, RuntimePayloadRepresentation, RuntimeTypeRepresentation,
    RuntimeTypeRepresentationTable, RuntimeVariantRepresentation,
};
pub use target::RuntimeAbiIdentity;
