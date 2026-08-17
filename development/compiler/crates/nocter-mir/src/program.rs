use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, BorrowCapability, BuiltinType, ExecutableItemId, MirPlaceId, MirValueId,
    TypeId, TypeKind,
};
use nocter_target_program::{ExecutableItemKey, ExecutableProgram};

use crate::{MirCallTarget, MirFunction, MirOperationKind, MirValidationError, validate_function};

/// One closed executable and exactly one validated MIR function per executable item.
#[derive(Debug)]
pub struct MirProgram {
    executable: ExecutableProgram,
    functions: Arena<ExecutableItemId, MirFunction>,
}

impl MirProgram {
    #[must_use]
    pub const fn executable(&self) -> &ExecutableProgram {
        &self.executable
    }

    #[must_use]
    pub const fn functions(&self) -> &Arena<ExecutableItemId, MirFunction> {
        &self.functions
    }
}

/// The sole mutable construction path for a [`MirProgram`].
#[derive(Debug)]
pub struct MirProgramBuilder {
    executable: ExecutableProgram,
    functions: ArenaBuilder<ExecutableItemId, Option<MirFunction>>,
}

impl MirProgramBuilder {
    #[must_use]
    pub fn new(executable: ExecutableProgram) -> Self {
        let mut functions = ArenaBuilder::new();
        for _ in executable.items().iter() {
            functions.insert(None);
        }
        Self {
            executable,
            functions,
        }
    }

    #[must_use]
    pub const fn executable(&self) -> &ExecutableProgram {
        &self.executable
    }

    /// Installs one function in its executable-item slot exactly once.
    ///
    /// # Errors
    ///
    /// Rejects an unknown identity, a mismatched function identity, or a duplicate definition.
    pub fn define(
        &mut self,
        item: ExecutableItemId,
        function: MirFunction,
    ) -> Result<(), MirProgramBuildError> {
        if function.item() != item {
            return Err(MirProgramBuildError::MismatchedItem {
                slot: item,
                function: function.item(),
            });
        }
        validate_function(&function, &self.executable)?;
        let slot = self
            .functions
            .get_mut(item)
            .ok_or(MirProgramBuildError::UnknownItem(item))?;
        if slot.replace(function).is_some() {
            return Err(MirProgramBuildError::DuplicateFunction(item));
        }
        Ok(())
    }

    /// Freezes the complete function arena and validates cross-function calls.
    ///
    /// # Errors
    ///
    /// Rejects a missing function or any direct-call signature mismatch.
    pub fn finish(self) -> Result<MirProgram, MirProgramBuildError> {
        let functions = self.functions.try_finish_with(|item, function| {
            function.ok_or(MirProgramBuildError::MissingFunction(item))
        })?;
        validate_cross_function_calls(&functions, &self.executable)?;
        Ok(MirProgram {
            executable: self.executable,
            functions,
        })
    }
}

fn validate_cross_function_calls(
    functions: &Arena<ExecutableItemId, MirFunction>,
    executable: &ExecutableProgram,
) -> Result<(), MirProgramBuildError> {
    for (caller, function) in functions.iter() {
        for (_, operation) in function.operations().iter() {
            match operation.kind() {
                MirOperationKind::Call(call) => {
                    let MirCallTarget::Direct(callee) = call.target() else {
                        continue;
                    };
                    let callee_function = functions
                        .get(*callee)
                        .ok_or(MirProgramBuildError::UnknownItem(*callee))?;
                    let parameter_types = callee_function
                        .parameters()
                        .iter()
                        .map(|parameter| {
                            callee_function
                                .locals()
                                .get(*parameter)
                                .copied()
                                .map(crate::MirLocal::ty)
                                .expect("validated MIR parameter must exist")
                        })
                        .collect::<Vec<_>>();
                    if parameter_types.len() != call.arguments().len()
                        || parameter_types
                            .iter()
                            .copied()
                            .zip(call.arguments().iter().copied())
                            .any(|(expected, argument)| {
                                value_type(function, argument) != Some(expected)
                            })
                        || operation
                            .result()
                            .and_then(|value| value_type(function, value))
                            != Some(callee_function.result())
                    {
                        return Err(MirProgramBuildError::DirectCallSignature {
                            caller,
                            callee: *callee,
                        });
                    }
                }
                MirOperationKind::InvokeDrop { body, place } => {
                    let callee = functions
                        .get(*body)
                        .ok_or(MirProgramBuildError::UnknownItem(*body))?;
                    let is_drop = matches!(
                        executable
                            .items()
                            .get(*body)
                            .map(nocter_target_program::ExecutableItem::key),
                        Some(ExecutableItemKey::Drop(_))
                    );
                    let place_type = place_type(function, *place);
                    let parameter_type = parameter_type(callee, 0);
                    if callee.parameters().len() != 1
                        || callee.result() != executable.types().builtin(BuiltinType::Void)
                        || !is_drop
                        || !matches!(
                            parameter_type.and_then(|ty| executable.types().get(ty)),
                            Some(TypeKind::Borrow {
                                capability: BorrowCapability::ReadWrite,
                                referent,
                            }) if Some(*referent) == place_type
                        )
                    {
                        return Err(MirProgramBuildError::DropCallSignature {
                            caller,
                            callee: *body,
                            place: *place,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn value_type(function: &MirFunction, value: MirValueId) -> Option<TypeId> {
    function
        .values()
        .get(value)
        .copied()
        .map(crate::MirValue::ty)
}

fn place_type(function: &MirFunction, place: MirPlaceId) -> Option<TypeId> {
    function.places().get(place).map(crate::MirPlace::ty)
}

fn parameter_type(function: &MirFunction, position: usize) -> Option<TypeId> {
    function
        .parameters()
        .get(position)
        .and_then(|parameter| function.locals().get(*parameter))
        .copied()
        .map(crate::MirLocal::ty)
}

#[derive(Debug)]
pub enum MirProgramBuildError {
    Validation(MirValidationError),
    UnknownItem(ExecutableItemId),
    MismatchedItem {
        slot: ExecutableItemId,
        function: ExecutableItemId,
    },
    DuplicateFunction(ExecutableItemId),
    MissingFunction(ExecutableItemId),
    DirectCallSignature {
        caller: ExecutableItemId,
        callee: ExecutableItemId,
    },
    DropCallSignature {
        caller: ExecutableItemId,
        callee: ExecutableItemId,
        place: MirPlaceId,
    },
}

impl fmt::Display for MirProgramBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MIR program construction failed: {self:?}")
    }
}

impl std::error::Error for MirProgramBuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::UnknownItem(_)
            | Self::MismatchedItem { .. }
            | Self::DuplicateFunction(_)
            | Self::MissingFunction(_)
            | Self::DirectCallSignature { .. }
            | Self::DropCallSignature { .. } => None,
        }
    }
}

impl From<MirValidationError> for MirProgramBuildError {
    fn from(error: MirValidationError) -> Self {
        Self::Validation(error)
    }
}
