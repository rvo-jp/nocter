use std::collections::BTreeSet;
use std::fmt;

use nocter_machine::{MachineCallTarget, MachineOperationKind};
use nocter_runtime_contract::RuntimeAbiIdentity;

use crate::{
    Arm64AsyncPrimitiveTargets, Arm64DarwinNetworkPrimitiveAbis, Arm64DarwinNetworkPrimitiveError,
    Arm64DarwinNetworkPrimitiveTargets, Arm64DarwinProcessServiceError,
    Arm64DarwinProcessServiceTargets, Arm64ProgramBuilder,
};

/// Every native helper family required by one machine program.
///
/// The machine operation graph is scanned exactly once. Individual helper families receive the
/// resulting closed role set and cannot rediscover primitive usage independently.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Arm64PrimitiveTargets {
    asynchronous: Arm64AsyncPrimitiveTargets,
    network: Option<Arm64DarwinNetworkPrimitiveTargets>,
    process: Option<Arm64DarwinProcessServiceTargets>,
}

impl Arm64PrimitiveTargets {
    pub(crate) fn declare(
        machine: &nocter_machine::MachineProgram,
        builder: &mut Arm64ProgramBuilder,
    ) -> Result<Self, Arm64PrimitiveTargetError> {
        let mut roles = BTreeSet::new();
        let mut network_abis = Arm64DarwinNetworkPrimitiveAbis::default();
        for target in machine.functions().flat_map(|(_, function)| {
            function.body().operations().filter_map(|(_, operation)| {
                let MachineOperationKind::Call(call) = operation.kind() else {
                    return None;
                };
                let MachineCallTarget::Primitive(target) = call.target() else {
                    return None;
                };
                Some(target)
            })
        }) {
            roles.insert(target.role());
            network_abis.remember(machine, target)?;
        }
        if roles
            .iter()
            .copied()
            .any(|role| crate::Arm64DarwinNetworkPrimitive::from_role(role).is_some())
            && machine.layouts().target().runtime_schema()
                != RuntimeAbiIdentity::Arm64DarwinV1.schema()
        {
            return Err(Arm64DarwinNetworkPrimitiveError::PrimitiveAbi.into());
        }
        Ok(Self {
            asynchronous: Arm64AsyncPrimitiveTargets::declare(&roles, builder),
            network: Arm64DarwinNetworkPrimitiveTargets::declare(
                machine,
                &roles,
                network_abis,
                builder,
            )?,
            process: Arm64DarwinProcessServiceTargets::declare(&roles, builder)?,
        })
    }

    pub(crate) const fn asynchronous(self) -> Arm64AsyncPrimitiveTargets {
        self.asynchronous
    }

    pub(crate) const fn network(self) -> Option<Arm64DarwinNetworkPrimitiveTargets> {
        self.network
    }

    pub(crate) const fn process(self) -> Option<Arm64DarwinProcessServiceTargets> {
        self.process
    }
}

/// Failure while declaring one compiler-owned primitive helper family.
#[derive(Debug)]
pub enum Arm64PrimitiveTargetError {
    Network(Arm64DarwinNetworkPrimitiveError),
    Process(Arm64DarwinProcessServiceError),
}

impl fmt::Display for Arm64PrimitiveTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ARM64 primitive target declaration failed: {self:?}"
        )
    }
}

impl std::error::Error for Arm64PrimitiveTargetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Network(error) => Some(error),
            Self::Process(error) => Some(error),
        }
    }
}

impl From<Arm64DarwinNetworkPrimitiveError> for Arm64PrimitiveTargetError {
    fn from(error: Arm64DarwinNetworkPrimitiveError) -> Self {
        Self::Network(error)
    }
}

impl From<Arm64DarwinProcessServiceError> for Arm64PrimitiveTargetError {
    fn from(error: Arm64DarwinProcessServiceError) -> Self {
        Self::Process(error)
    }
}
