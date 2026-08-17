//! Validated, backend-independent control-flow representation.
//!
//! MIR owns concrete places, values, operations, blocks, and terminal control flow. It consumes
//! executable semantic identities and never retains syntax, source ranges, unresolved dispatch,
//! or rendered type names.

mod builder;
mod destruction;
mod lower;
mod operation;
mod pack;
mod place;
mod program;
mod program_validation;
mod schema;
mod validate;
mod validation_call;
mod validation_closure;
mod validation_destruction;
mod validation_environment;
mod validation_error;
mod validation_graph;
mod validation_pack;
mod validation_place;
mod validation_region;
mod validation_switch;
mod validation_types;

pub use builder::{MirBlockBuilder, MirFunctionBuildError, MirFunctionBuilder};
pub use destruction::{
    MirCaptureDestruction, MirDestructionKind, MirDestructionPlan, MirFieldDestruction,
    MirPayloadDestruction, MirVariantDestruction,
};
pub use lower::{MirLoweringError, lower_executable};
pub use operation::{
    MirAggregate, MirBinaryOperation, MirCall, MirCallAllocation, MirCallSignature, MirCallTarget,
    MirClosureCapture, MirConstant, MirOperation, MirOperationKind, MirReadMode, MirStructuralCall,
    MirUnaryOperation,
};
pub use pack::{
    MirPackArgument, MirPackContribution, MirPackInput, MirPackNext, MirPackSegment, MirPackSpread,
};
pub use place::{MirLocal, MirLocalKind, MirPlace, MirPlaceRoot, MirProjection, MirProjectionKind};
pub use program::{MirProgram, MirProgramBuildError, MirProgramBuilder};
pub use schema::{
    MirBlock, MirBranchTarget, MirDropFlag, MirFunction, MirSwitchCase, MirSwitchSubject,
    MirSwitchValue, MirTerminator, MirValue, MirValueDefinition,
};
pub use validate::validate_function;
pub use validation_environment::MirValidationEnvironment;
pub use validation_error::MirValidationError;

#[cfg(test)]
mod tests;
