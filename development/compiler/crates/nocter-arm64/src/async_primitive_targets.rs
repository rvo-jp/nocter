use nocter_machine::{MachineCallTarget, MachineOperationKind};
use nocter_runtime_contract::PrimitiveRole;

use crate::{Arm64FunctionId, Arm64ProgramBuilder};

/// Shared native lifecycle entries for compiler-owned single-interest computations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncInterestLifecycleTargets {
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
}

impl Arm64AsyncInterestLifecycleTargets {
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

/// Native helper identities selected once from the machine program's primitive dependencies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Arm64AsyncPrimitiveTargets {
    descriptor_readiness: Option<Arm64FunctionId>,
    monotonic_deadline: Option<Arm64FunctionId>,
    interest_lifecycle: Option<Arm64AsyncInterestLifecycleTargets>,
}

impl Arm64AsyncPrimitiveTargets {
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        builder: &mut Arm64ProgramBuilder,
    ) -> Self {
        let mut descriptor_readiness = false;
        let mut monotonic_deadline = false;
        for role in machine.functions().flat_map(|(_, function)| {
            function.body().operations().filter_map(|(_, operation)| {
                let MachineOperationKind::Call(call) = operation.kind() else {
                    return None;
                };
                let MachineCallTarget::Primitive(target) = call.target() else {
                    return None;
                };
                Some(target.role())
            })
        }) {
            descriptor_readiness |= role == PrimitiveRole::DescriptorReadiness;
            monotonic_deadline |= role == PrimitiveRole::MonotonicDeadline;
        }
        let interest_lifecycle = (descriptor_readiness || monotonic_deadline).then(|| {
            Arm64AsyncInterestLifecycleTargets {
                resume: builder.declare_function(),
                cancel: builder.declare_function(),
                consume: builder.declare_function(),
            }
        });
        Self {
            descriptor_readiness: descriptor_readiness.then(|| builder.declare_function()),
            monotonic_deadline: monotonic_deadline.then(|| builder.declare_function()),
            interest_lifecycle,
        }
    }

    #[must_use]
    pub const fn descriptor_readiness(self) -> Option<Arm64FunctionId> {
        self.descriptor_readiness
    }

    #[must_use]
    pub const fn monotonic_deadline(self) -> Option<Arm64FunctionId> {
        self.monotonic_deadline
    }

    #[must_use]
    pub const fn interest_lifecycle(self) -> Option<Arm64AsyncInterestLifecycleTargets> {
        self.interest_lifecycle
    }
}
