use std::fmt;

use nocter_machine::{MachineFunctionExecution, MachineFunctionId};

use crate::{Arm64FunctionId, Arm64ProgramBuilder};

/// Native lifecycle entries owned by one deferred Machine function.
///
/// The callable entry constructs the owning computation and therefore remains on
/// [`Arm64FunctionTarget`]. These entries operate on the resulting opaque frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncFunctionTargets {
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
}

impl Arm64AsyncFunctionTargets {
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
}

/// Complete native-entry identity assigned to one Machine function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64FunctionTarget {
    owner: MachineFunctionId,
    callable: Arm64FunctionId,
    asynchronous: Option<Arm64AsyncFunctionTargets>,
}

impl Arm64FunctionTarget {
    #[must_use]
    pub const fn owner(self) -> MachineFunctionId {
        self.owner
    }

    /// Returns the only target used by an ordinary Machine call.
    ///
    /// This is the body entry for an immediate function and the computation constructor for a
    /// deferred function. Call materialization therefore does not need to know execution kind.
    #[must_use]
    pub const fn callable(self) -> Arm64FunctionId {
        self.callable
    }

    #[must_use]
    pub const fn asynchronous(self) -> Option<Arm64AsyncFunctionTargets> {
        self.asynchronous
    }
}

/// Dense, immutable authority for every Machine-function to native-entry mapping.
///
/// Native identities are declared together before any body is materialized. Direct calls and
/// generated helpers consume this table instead of reconstructing execution-kind decisions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64FunctionTargets {
    entries: Box<[Arm64FunctionTarget]>,
}

impl Arm64FunctionTargets {
    /// Declares the complete native identity domain for a Machine program.
    ///
    /// # Errors
    ///
    /// Rejects a non-dense Machine function domain.
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        builder: &mut Arm64ProgramBuilder,
    ) -> Result<Self, Arm64FunctionTargetsError> {
        let mut entries = Vec::with_capacity(machine.functions().len());
        for (owner, function) in machine.functions() {
            if owner.index() != entries.len() {
                return Err(Arm64FunctionTargetsError::NonDenseFunction(owner));
            }
            let callable = builder.declare_function();
            let asynchronous = match function.execution() {
                MachineFunctionExecution::Immediate => None,
                MachineFunctionExecution::Deferred(_) => Some(Arm64AsyncFunctionTargets {
                    resume: builder.declare_function(),
                    cancel: builder.declare_function(),
                    consume: builder.declare_function(),
                }),
            };
            entries.push(Arm64FunctionTarget {
                owner,
                callable,
                asynchronous,
            });
        }
        Ok(Self {
            entries: entries.into_boxed_slice(),
        })
    }

    #[must_use]
    pub fn get(&self, owner: MachineFunctionId) -> Option<Arm64FunctionTarget> {
        self.entries
            .get(owner.index())
            .copied()
            .filter(|target| target.owner == owner)
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Arm64FunctionTarget> + '_ {
        self.entries.iter().copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64FunctionTargetsError {
    NonDenseFunction(MachineFunctionId),
}

impl fmt::Display for Arm64FunctionTargetsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 function-target planning failed: {self:?}")
    }
}

impl std::error::Error for Arm64FunctionTargetsError {}
