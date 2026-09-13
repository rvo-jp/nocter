use std::fmt;

use nocter_runtime_contract::DarwinFileOperation;

use crate::{
    Arm64CodeError, Arm64DarwinFileLifecycleError, Arm64FunctionId, Arm64ProgramBuilder,
    Arm64ProgramError,
};

/// Complete generated target family for owned Darwin file-operation computations.
///
/// Construction, drive, cancellation, consumption, and worker publication are declared together;
/// no external caller can assemble callbacks from unrelated target families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DarwinFileJobTargets {
    constructors: [Arm64FunctionId; DarwinFileOperation::ALL.len()],
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
    worker: Arm64FunctionId,
}

impl Arm64DarwinFileJobTargets {
    /// Declares and defines the complete operation-job family.
    ///
    /// # Errors
    ///
    /// Propagates invalid target contracts, lifecycle, ARM64 encoding, and program errors.
    pub fn declare(
        program: &mut Arm64ProgramBuilder,
        imports: &crate::Arm64DarwinFileServiceImports,
        root: crate::Arm64DarwinFileServiceRootTargets,
        retirement: crate::Arm64DarwinFileRetirementTargets,
    ) -> Result<Self, Arm64DarwinFileJobError> {
        let constructors = std::array::from_fn(|_| program.declare_function());
        let targets = Self {
            constructors,
            resume: program.declare_function(),
            cancel: program.declare_function(),
            consume: program.declare_function(),
            worker: program.declare_function(),
        };
        for operation in DarwinFileOperation::ALL.iter().copied() {
            program.define_function(
                targets.constructor(operation),
                crate::darwin_file_job_constructor::build(operation, targets, imports, root)?
                    .finish()?,
            )?;
        }
        program.define_function(
            targets.resume,
            crate::darwin_file_job_execution::build_resume(targets, imports, retirement)?
                .finish()?,
        )?;
        program.define_function(
            targets.cancel,
            crate::darwin_file_job_execution::build_cancel(imports, retirement)?.finish()?,
        )?;
        program.define_function(
            targets.consume,
            crate::darwin_file_job_execution::build_consume(imports)?.finish()?,
        )?;
        program.define_function(
            targets.worker,
            crate::darwin_file_job_execution::build_worker(imports, retirement)?.finish()?,
        )?;
        Ok(targets)
    }

    #[must_use]
    pub const fn constructor(self, operation: DarwinFileOperation) -> Arm64FunctionId {
        self.constructors[operation.code() as usize]
    }

    #[must_use]
    pub const fn resume(self) -> Arm64FunctionId {
        self.resume
    }

    #[must_use]
    pub const fn cancel(self) -> Arm64FunctionId {
        self.cancel
    }

    #[must_use]
    pub const fn consume(self) -> Arm64FunctionId {
        self.consume
    }

    #[must_use]
    pub const fn worker(self) -> Arm64FunctionId {
        self.worker
    }
}

#[derive(Debug)]
pub enum Arm64DarwinFileJobError {
    ContractLayout,
    LifecycleAction,
    Lifecycle(Arm64DarwinFileLifecycleError),
    Code(Arm64CodeError),
    Program(Arm64ProgramError),
}

impl fmt::Display for Arm64DarwinFileJobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 Darwin file job failed: {self:?}")
    }
}

impl std::error::Error for Arm64DarwinFileJobError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Code(error) => Some(error),
            Self::Program(error) => Some(error),
            Self::ContractLayout | Self::LifecycleAction => None,
        }
    }
}

impl From<Arm64DarwinFileLifecycleError> for Arm64DarwinFileJobError {
    fn from(error: Arm64DarwinFileLifecycleError) -> Self {
        Self::Lifecycle(error)
    }
}

impl From<Arm64CodeError> for Arm64DarwinFileJobError {
    fn from(error: Arm64CodeError) -> Self {
        Self::Code(error)
    }
}

impl From<Arm64ProgramError> for Arm64DarwinFileJobError {
    fn from(error: Arm64ProgramError) -> Self {
        Self::Program(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_runtime_contract::{DarwinFileOperation, DarwinFileServiceFunction};

    use super::Arm64DarwinFileJobTargets;
    use crate::{
        Arm64DarwinFileRetirementTargets, Arm64DarwinFileServiceImports,
        Arm64DarwinFileServiceRootTargets, Arm64ProgramBuilder,
    };

    #[test]
    fn job_targets_define_every_constructor_and_one_shared_lifecycle() {
        let mut program = Arm64ProgramBuilder::new();
        let imports = Arm64DarwinFileServiceImports::declare(&mut program).unwrap();
        let retirement = Arm64DarwinFileRetirementTargets::declare(&mut program, &imports).unwrap();
        let root =
            Arm64DarwinFileServiceRootTargets::declare(&mut program, &imports, retirement).unwrap();
        let jobs =
            Arm64DarwinFileJobTargets::declare(&mut program, &imports, root, retirement).unwrap();
        program
            .set_entry(jobs.constructor(DarwinFileOperation::Open))
            .unwrap();
        let program = program.finish().unwrap();
        for operation in DarwinFileOperation::ALL {
            assert!(
                program
                    .function(jobs.constructor(*operation))
                    .unwrap()
                    .size()
                    > 0
            );
        }
        for target in [jobs.resume(), jobs.cancel(), jobs.consume(), jobs.worker()] {
            assert!(program.function(target).unwrap().size() > 0);
        }
        assert_eq!(
            program.runtime_imports().len(),
            DarwinFileServiceFunction::ALL.len()
        );
    }
}
