use std::fmt;

use nocter_model::{Arena, ArenaBuilder, ExecutableItemId, MirPlaceId, TestId, TypeId};
use nocter_runtime_contract::{
    RuntimeAbiIdentity, RuntimeEnvironment, RuntimeTypeRepresentationTable, RuntimeTypeTable,
};
use nocter_target_program::{ExecutableProgram, ExecutableRoot};

use crate::program_validation::validate_program;
use crate::validate::validate_root;
use crate::{MirFunction, MirRoot, MirValidationError, validate_function};

/// One closed executable with one function per executable item and its compiler-owned roots.
#[derive(Debug)]
pub struct MirProgram {
    runtime: RuntimeEnvironment,
    functions: Arena<ExecutableItemId, MirFunction>,
    root: MirRoot,
}

impl MirProgram {
    #[must_use]
    pub const fn types(&self) -> &RuntimeTypeTable {
        self.runtime.types()
    }

    #[must_use]
    pub const fn type_representations(&self) -> &RuntimeTypeRepresentationTable {
        self.runtime.type_representations()
    }

    #[must_use]
    pub const fn runtime_abi(&self) -> RuntimeAbiIdentity {
        self.runtime.abi()
    }

    #[must_use]
    pub const fn functions(&self) -> &Arena<ExecutableItemId, MirFunction> {
        &self.functions
    }

    #[must_use]
    pub const fn root(&self) -> &MirRoot {
        &self.root
    }
}

/// The sole mutable construction path for a [`MirProgram`].
#[derive(Debug)]
pub struct MirProgramBuilder {
    executable: ExecutableProgram,
    functions: ArenaBuilder<ExecutableItemId, Option<MirFunction>>,
    root: Option<MirRoot>,
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
            root: None,
        }
    }

    /// Installs the compiler-owned root set exactly once.
    ///
    /// # Errors
    ///
    /// Rejects a root whose identities differ from the executable closure or whose bodies violate
    /// root-local MIR invariants.
    pub fn define_root(&mut self, root: MirRoot) -> Result<(), MirProgramBuildError> {
        if self.root.is_some() {
            return Err(MirProgramBuildError::DuplicateRoot);
        }
        validate_root_metadata(&root, self.executable.root())?;
        match &root {
            MirRoot::Process(process) => validate_root(process.body(), &self.executable)?,
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    validate_root(case.body(), &self.executable)?;
                }
            }
        }
        self.root = Some(root);
        Ok(())
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

    /// Freezes the complete program and validates every cross-body call.
    ///
    /// # Errors
    ///
    /// Rejects a missing function/root or any direct-call signature mismatch.
    pub fn finish(self) -> Result<MirProgram, MirProgramBuildError> {
        let functions = self.functions.try_finish_with(|item, function| {
            function.ok_or(MirProgramBuildError::MissingFunction(item))
        })?;
        let root = self.root.ok_or(MirProgramBuildError::MissingRoot)?;
        validate_program(&functions, &root, &self.executable)?;
        let runtime = self.executable.into_runtime_environment();
        Ok(MirProgram {
            runtime,
            functions,
            root,
        })
    }
}

fn validate_root_metadata(
    root: &MirRoot,
    executable: &ExecutableRoot,
) -> Result<(), MirProgramBuildError> {
    let matches = match (root, executable) {
        (
            MirRoot::Process(root),
            ExecutableRoot::Process {
                target,
                entry,
                result,
            },
        ) => root.target() == *target && root.entry() == *entry && root.result() == *result,
        (
            MirRoot::Tests {
                target: root_target,
                cases: root_cases,
            },
            ExecutableRoot::Tests { target, cases },
        ) => {
            root_target == target
                && root_cases.len() == cases.len()
                && root_cases.iter().zip(cases).all(|(root, case)| {
                    root.declaration() == case.declaration()
                        && root.name() == case.name()
                        && root.item() == case.item()
                })
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(MirProgramBuildError::MismatchedRoot)
    }
}

/// One exact owner used when validating calls that cross MIR body boundaries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirProgramOwner {
    Function(ExecutableItemId),
    ProcessRoot,
    TestRoot(TestId),
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
    DuplicateRoot,
    MissingRoot,
    MismatchedRoot,
    InvalidRootCall {
        owner: MirProgramOwner,
        expected: ExecutableItemId,
    },
    DirectCallSignature {
        caller: MirProgramOwner,
        callee: ExecutableItemId,
    },
    DropCallSignature {
        caller: MirProgramOwner,
        callee: ExecutableItemId,
        place: MirPlaceId,
    },
    DeferredDropSignature {
        caller: MirProgramOwner,
        callee: ExecutableItemId,
        ty: TypeId,
    },
    ClosureConstructionSignature {
        caller: MirProgramOwner,
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
            | Self::DuplicateRoot
            | Self::MissingRoot
            | Self::MismatchedRoot
            | Self::InvalidRootCall { .. }
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
