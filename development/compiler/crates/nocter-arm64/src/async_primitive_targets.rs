use nocter_machine::{MachineCallTarget, MachineOperationKind};
use nocter_runtime_contract::PrimitiveRole;

use crate::{Arm64FunctionId, Arm64ProgramBuilder};

/// Shared native lifecycle entries for the descriptor-readiness computation primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64DescriptorReadinessTargets {
    constructor: Arm64FunctionId,
    resume: Arm64FunctionId,
    cancel: Arm64FunctionId,
    consume: Arm64FunctionId,
}

impl Arm64DescriptorReadinessTargets {
    #[must_use]
    pub const fn constructor(self) -> Arm64FunctionId {
        self.constructor
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
}

/// Native helper identities selected once from the machine program's primitive dependencies.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Arm64AsyncPrimitiveTargets {
    descriptor_readiness: Option<Arm64DescriptorReadinessTargets>,
}

impl Arm64AsyncPrimitiveTargets {
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        builder: &mut Arm64ProgramBuilder,
    ) -> Self {
        let required = machine.functions().any(|(_, function)| {
            function.body().operations().any(|(_, operation)| {
                matches!(
                    operation.kind(),
                    MachineOperationKind::Call(call)
                        if matches!(
                            call.target(),
                            MachineCallTarget::Primitive(target)
                                if target.role() == PrimitiveRole::DescriptorReadiness
                        )
                )
            })
        });
        Self {
            descriptor_readiness: required.then(|| Arm64DescriptorReadinessTargets {
                constructor: builder.declare_function(),
                resume: builder.declare_function(),
                cancel: builder.declare_function(),
                consume: builder.declare_function(),
            }),
        }
    }

    #[must_use]
    pub const fn descriptor_readiness(self) -> Option<Arm64DescriptorReadinessTargets> {
        self.descriptor_readiness
    }
}
