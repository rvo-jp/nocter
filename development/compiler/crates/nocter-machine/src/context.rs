use std::fmt;

use nocter_model::TypeId;
use nocter_runtime_contract::PrimitiveRole;

use crate::identity::MachineTable;
use crate::{
    MachineCall, MachineCallAllocation, MachineCallTarget, MachineFunction, MachineFunctionId,
    MachineFunctionKind, MachineOperationKind, MachinePack, MachinePackId, MachinePackSegment,
    MachinePrimitiveDependency,
};

/// One compiler-propagated ambient capability with a dedicated hidden ABI lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineContextKind {
    Allocation,
    Process,
}

/// How one machine function obtains an ambient compiler context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineContextRequirement {
    None,
    Incoming,
    ProgramRoot,
}

/// Whole-program least fixed point for one ambient capability.
#[derive(Debug)]
pub struct MachineContextPlan {
    kind: MachineContextKind,
    functions: MachineTable<MachineFunctionId, MachineContextRequirement>,
}

impl MachineContextPlan {
    fn build(
        kind: MachineContextKind,
        functions: &MachineTable<MachineFunctionId, MachineFunction>,
    ) -> Result<Self, MachineContextError> {
        let mut requirements = functions
            .iter()
            .map(|(_, function)| match (kind, function.kind()) {
                (
                    MachineContextKind::Allocation,
                    MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. },
                ) => MachineContextRequirement::ProgramRoot,
                (
                    MachineContextKind::Process | MachineContextKind::Allocation,
                    MachineFunctionKind::Callable(_),
                )
                | (
                    MachineContextKind::Process,
                    MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. },
                ) => MachineContextRequirement::None,
            })
            .collect::<Vec<_>>();

        loop {
            let previous = requirements.clone();
            propagate_pack_callbacks(kind, functions, &previous, &mut requirements)?;
            propagate_function_bodies(kind, functions, &previous, &mut requirements)?;
            if requirements == previous {
                break;
            }
        }

        Ok(Self {
            kind,
            functions: MachineTable::from_values(requirements),
        })
    }

    #[must_use]
    pub const fn kind(&self) -> MachineContextKind {
        self.kind
    }

    #[must_use]
    pub fn get(&self, function: MachineFunctionId) -> Option<MachineContextRequirement> {
        self.functions.get(function).copied()
    }

    /// Whether this exact target consumes this plan's hidden context lane.
    ///
    /// # Errors
    ///
    /// Rejects a direct target outside the completed function domain or an unclosed primitive
    /// destruction dependency.
    pub fn target_requires_context(
        &self,
        target: &MachineCallTarget,
    ) -> Result<bool, MachineContextError> {
        target_requires_context(self.kind, target, self.functions.values())
    }

    /// Whether this call requires the caller's own incoming lane. Explicit and lexical allocation
    /// selection satisfy only the allocation capability locally; process state is always ambient.
    ///
    /// # Errors
    ///
    /// Rejects a target outside the completed context graph.
    pub fn call_requires_incoming(&self, call: &MachineCall) -> Result<bool, MachineContextError> {
        let required = self.target_requires_context(call.target())?;
        Ok(required
            && (self.kind == MachineContextKind::Process
                || call.allocation() == MachineCallAllocation::Inherit))
    }
}

/// All ambient capabilities frozen together before target lowering.
#[derive(Debug)]
pub struct MachineContextPlans {
    allocation: MachineContextPlan,
    process: MachineContextPlan,
}

impl MachineContextPlans {
    pub(crate) fn build(
        functions: &MachineTable<MachineFunctionId, MachineFunction>,
    ) -> Result<Self, MachineContextError> {
        Ok(Self {
            allocation: MachineContextPlan::build(MachineContextKind::Allocation, functions)?,
            process: MachineContextPlan::build(MachineContextKind::Process, functions)?,
        })
    }

    #[must_use]
    pub const fn allocation(&self) -> &MachineContextPlan {
        &self.allocation
    }

    #[must_use]
    pub const fn process(&self) -> &MachineContextPlan {
        &self.process
    }
}

fn propagate_pack_callbacks(
    kind: MachineContextKind,
    functions: &MachineTable<MachineFunctionId, MachineFunction>,
    previous: &[MachineContextRequirement],
    requirements: &mut [MachineContextRequirement],
) -> Result<(), MachineContextError> {
    for (function_id, function) in functions.iter() {
        for (_, operation) in function.body().operations() {
            let MachineOperationKind::Call(call) = operation.kind() else {
                continue;
            };
            let (Some(pack), MachineCallTarget::Direct(target)) = (call.pack(), call.target())
            else {
                continue;
            };
            let pack = function
                .body()
                .pack(pack)
                .ok_or(MachineContextError::UnknownPack {
                    kind,
                    function: function_id,
                    pack,
                })?;
            if pack_requires_context(kind, pack, previous)? {
                mark_requirement(functions, requirements, *target, kind)?;
            }
        }
    }
    Ok(())
}

fn propagate_function_bodies(
    kind: MachineContextKind,
    functions: &MachineTable<MachineFunctionId, MachineFunction>,
    previous: &[MachineContextRequirement],
    requirements: &mut [MachineContextRequirement],
) -> Result<(), MachineContextError> {
    for (function_id, function) in functions.iter() {
        for (_, operation) in function.body().operations() {
            if operation_requires_context(kind, operation.kind(), previous)? {
                mark_requirement(functions, requirements, function_id, kind)?;
                break;
            }
        }
    }
    Ok(())
}

fn operation_requires_context(
    kind: MachineContextKind,
    operation: &MachineOperationKind,
    requirements: &[MachineContextRequirement],
) -> Result<bool, MachineContextError> {
    match operation {
        MachineOperationKind::Call(call) => {
            let target = target_requires_context(kind, call.target(), requirements)?;
            Ok(target
                && (kind == MachineContextKind::Process
                    || call.allocation() == MachineCallAllocation::Inherit))
        }
        MachineOperationKind::InvokeDrop {
            target, allocation, ..
        } => Ok(function_requires_context(kind, requirements, *target)?
            && (kind == MachineContextKind::Process
                || *allocation == MachineCallAllocation::Inherit)),
        _ => Ok(false),
    }
}

fn pack_requires_context(
    kind: MachineContextKind,
    pack: &MachinePack,
    requirements: &[MachineContextRequirement],
) -> Result<bool, MachineContextError> {
    for segment in pack.segments() {
        match segment {
            MachinePackSegment::Value { destruction, .. } => {
                if let Some(function) = destruction
                    && function_requires_context(kind, requirements, *function)?
                {
                    return Ok(true);
                }
            }
            MachinePackSegment::Spread(spread) => {
                if function_requires_context(kind, requirements, spread.next().target())? {
                    return Ok(true);
                }
                if let Some(function) = spread.destruction()
                    && function_requires_context(kind, requirements, function)?
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn target_requires_context(
    kind: MachineContextKind,
    target: &MachineCallTarget,
    requirements: &[MachineContextRequirement],
) -> Result<bool, MachineContextError> {
    match target {
        MachineCallTarget::Direct(function) => {
            function_requires_context(kind, requirements, *function)
        }
        MachineCallTarget::Primitive(primitive) => {
            if let MachinePrimitiveDependency::Destruction {
                plan: Some(plan), ..
            } = primitive.dependency()
            {
                return Err(MachineContextError::UnloweredDestructionPlan {
                    kind,
                    ty: plan.ty(),
                });
            }
            Ok(primitive_uses_context(kind, primitive.role()))
        }
    }
}

fn primitive_uses_context(kind: MachineContextKind, role: PrimitiveRole) -> bool {
    match kind {
        MachineContextKind::Allocation => matches!(
            role,
            PrimitiveRole::CurrentAllocatorState | PrimitiveRole::CurrentAllocatorKind
        ),
        MachineContextKind::Process => matches!(
            role,
            PrimitiveRole::ProcessArgumentCount
                | PrimitiveRole::ProcessArgument
                | PrimitiveRole::ProcessEnvironmentCount
                | PrimitiveRole::ProcessEnvironmentName
                | PrimitiveRole::ProcessEnvironmentValue
        ),
    }
}

fn function_requires_context(
    kind: MachineContextKind,
    requirements: &[MachineContextRequirement],
    function: MachineFunctionId,
) -> Result<bool, MachineContextError> {
    requirements
        .get(function.index())
        .copied()
        .map(|requirement| requirement != MachineContextRequirement::None)
        .ok_or(MachineContextError::UnknownFunction { kind, function })
}

fn mark_requirement(
    functions: &MachineTable<MachineFunctionId, MachineFunction>,
    requirements: &mut [MachineContextRequirement],
    function: MachineFunctionId,
    kind: MachineContextKind,
) -> Result<(), MachineContextError> {
    let requirement = requirements
        .get_mut(function.index())
        .ok_or(MachineContextError::UnknownFunction { kind, function })?;
    if *requirement != MachineContextRequirement::None {
        return Ok(());
    }
    let function_kind = functions
        .get(function)
        .map(MachineFunction::kind)
        .ok_or(MachineContextError::UnknownFunction { kind, function })?;
    *requirement = match function_kind {
        MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. } => {
            MachineContextRequirement::ProgramRoot
        }
        MachineFunctionKind::Callable(_) => MachineContextRequirement::Incoming,
    };
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineContextError {
    UnknownFunction {
        kind: MachineContextKind,
        function: MachineFunctionId,
    },
    UnloweredDestructionPlan {
        kind: MachineContextKind,
        ty: TypeId,
    },
    UnknownPack {
        kind: MachineContextKind,
        function: MachineFunctionId,
        pack: MachinePackId,
    },
}

impl fmt::Display for MachineContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid machine ambient-context graph: {self:?}")
    }
}

impl std::error::Error for MachineContextError {}
