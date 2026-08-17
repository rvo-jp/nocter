//! Target-independent machine-program and ABI layout authority.
//!
//! This crate consumes only validated MIR and the closed toolchain identities retained by that
//! program. It never receives syntax, name resolution, generic requirements, or rendered types.

mod layout;
mod target;
mod transport;

pub use layout::{
    MachineCaptureLayout, MachineEnumVariantLayout, MachineFieldLayout, MachineLayout,
    MachineLayoutError, MachineLayoutKind, MachineLayoutStore, MachineOutcomeKind,
    MachinePayloadLayout, MachineScalar,
};
pub use target::{MachineEndianness, MachineTarget};
pub use transport::{
    MachineAbiError, MachineAbiPlan, MachineArgumentAbi, MachineArgumentLocation,
    MachineCallableAbi, MachinePackAbi, MachineRegisterSpan, MachineResultAbi,
    MachineResultLocation, MachineReturnedValue, MachineStackSlot, MachineValueClass,
};

#[cfg(test)]
mod tests;
