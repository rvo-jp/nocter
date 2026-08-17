use std::fmt;

use nocter_model::{Arena, ArenaBuilder, ExecutableItemId, MirPlaceId, TypeId};
use nocter_target_program::ExecutableProgram;

use crate::program_validation::validate_program;
use crate::{MirFunction, MirValidationError, validate_function};

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
        validate_program(&functions, &self.executable)?;
        Ok(MirProgram {
            executable: self.executable,
            functions,
        })
    }
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
    DeferredDropSignature {
        caller: ExecutableItemId,
        callee: ExecutableItemId,
        ty: TypeId,
    },
    ClosureConstructionSignature {
        caller: ExecutableItemId,
        body: ExecutableItemId,
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
            | Self::DropCallSignature { .. }
            | Self::DeferredDropSignature { .. }
            | Self::ClosureConstructionSignature { .. } => None,
        }
    }
}

impl From<MirValidationError> for MirProgramBuildError {
    fn from(error: MirValidationError) -> Self {
        Self::Validation(error)
    }
}
