use std::fmt;

use nocter_model::{
    BodyId, BodyNodeId, ExecutableItemId, LocalBindingId, LoopId, ParameterId, PlaceId, TypeId,
};
use nocter_target_program::ExecutableProgram;

use crate::{MirBodyBuildError, MirProgram, MirProgramBuildError, MirProgramBuilder};

mod allocation;
mod argument_pack;
mod assignment;
mod binding;
mod borrow_conversion;
mod call;
mod callable_environment;
mod callable_value;
mod cleanup;
mod cleanup_flags;
mod closure;
mod collection_loop;
mod comparison;
mod control;
mod destruction_plan;
mod function;
mod index_place;
mod interpolation;
mod iteration;
mod loop_control;
mod opaque;
mod operand;
mod outcome;
mod pack_literal;
mod pattern;
mod place;
mod region;
mod root;
mod string;
mod value_storage;

#[cfg(test)]
mod tests;

/// Lowers one closed executable program without reopening semantic selection.
///
/// # Errors
///
/// Returns an integrity error when frozen executable facts are incomplete or inconsistent with
/// checked HIR, or while the current vertical lowering slice encounters an operation not yet
/// admitted to MIR.
pub fn lower_executable(executable: ExecutableProgram) -> Result<MirProgram, MirLoweringError> {
    let functions = executable
        .items()
        .iter()
        .map(|(item, definition)| {
            function::lower_function(&executable, item, definition).map(|function| (item, function))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = root::lower_root(&executable)?;
    let mut program = MirProgramBuilder::new(executable);
    for (item, function) in functions {
        program.define(item, function)?;
    }
    program.define_root(root)?;
    program.finish().map_err(Into::into)
}

#[derive(Debug)]
pub enum MirLoweringError {
    UnknownBody(BodyId),
    UnknownNode(BodyNodeId),
    UnknownPlace(PlaceId),
    UnknownLocal(LocalBindingId),
    InvalidLoop(LoopId),
    UnknownParameter(ParameterId),
    MissingConcreteType(TypeId),
    MissingInput(ParameterId),
    MissingValue(BodyNodeId),
    UnknownValue(nocter_model::MirValueId),
    MissingCurrentBlock,
    InvalidProjectionTypes(PlaceId),
    UnsupportedOperation(BodyNodeId),
    UnsupportedCleanup(BodyNodeId),
    UnsupportedPlaceProjection(PlaceId),
    ExpectedPlace(BodyNodeId),
    InvalidDispatch(BodyNodeId),
    InvalidPlaceDispatch(PlaceId),
    InvalidCleanup(BodyNodeId),
    InvalidPattern(BodyNodeId),
    InvalidRegion(BodyNodeId),
    InvalidClosure(BodyNodeId),
    InvalidCallable(BodyNodeId),
    InvalidInterpolation(BodyNodeId),
    InvalidStringLiteral(BodyNodeId),
    InvalidOpaqueWitness(BodyNodeId),
    InvalidCapture(nocter_model::CaptureId),
    InvalidTerminalResult(ExecutableItemId),
    InvalidRootItem(ExecutableItemId),
    InvalidOutcome(BodyNodeId),
    Body(MirBodyBuildError),
    Program(MirProgramBuildError),
}

impl fmt::Display for MirLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR lowering failed: {self:?}")
    }
}

impl std::error::Error for MirLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Body(error) => Some(error),
            Self::Program(error) => Some(error),
            _ => None,
        }
    }
}

impl From<MirBodyBuildError> for MirLoweringError {
    fn from(error: MirBodyBuildError) -> Self {
        Self::Body(error)
    }
}

impl From<MirProgramBuildError> for MirLoweringError {
    fn from(error: MirProgramBuildError) -> Self {
        Self::Program(error)
    }
}
