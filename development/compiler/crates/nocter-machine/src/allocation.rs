use std::fmt;

use nocter_target_program::PrimitiveRole;

use crate::identity::{MachineId, MachineTable};
use crate::{
    MachineCall, MachineCallAllocation, MachineCallTarget, MachineDestructionCapture,
    MachineDestructionField, MachineDestructionKind, MachineDestructionPayload,
    MachineDestructionPlan, MachineDestructionVariant, MachineFunction, MachineFunctionId,
    MachineFunctionKind, MachineOperationKind, MachinePack, MachinePackId, MachinePackSegment,
};

/// How one machine function obtains the compiler-propagated allocation context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAllocationRequirement {
    None,
    /// The caller supplies the current or explicitly selected context through the hidden ABI lane.
    Incoming,
    /// A compiler-owned process or test root establishes the program-lifetime default context.
    ProgramRoot,
}

/// Whole-program fixed point for the hidden execution-allocation capability.
#[derive(Debug)]
pub struct MachineAllocationPlan {
    functions: MachineTable<MachineFunctionId, MachineAllocationRequirement>,
}

impl MachineAllocationPlan {
    pub(crate) fn build(
        functions: &MachineTable<MachineFunctionId, MachineFunction>,
    ) -> Result<Self, MachineAllocationError> {
        let mut requirements = functions
            .iter()
            .map(|(_, function)| match function.kind() {
                MachineFunctionKind::ProcessRoot | MachineFunctionKind::TestRoot { .. } => {
                    MachineAllocationRequirement::ProgramRoot
                }
                MachineFunctionKind::Callable(_) => MachineAllocationRequirement::None,
            })
            .collect::<Vec<_>>();

        loop {
            let previous = requirements.clone();
            propagate_pack_callbacks(functions, &previous, &mut requirements)?;
            propagate_function_bodies(functions, &previous, &mut requirements)?;
            if requirements == previous {
                break;
            }
        }

        Ok(Self {
            functions: MachineTable::from_values(requirements),
        })
    }

    #[must_use]
    pub fn get(&self, function: MachineFunctionId) -> Option<MachineAllocationRequirement> {
        self.functions.get(function).copied()
    }

    /// Whether this exact call target consumes the hidden context lane.
    ///
    /// # Errors
    ///
    /// Rejects a direct target outside the completed machine function domain.
    pub fn target_requires_context(
        &self,
        target: &MachineCallTarget,
    ) -> Result<bool, MachineAllocationError> {
        target_requires_context(target, self.functions.values())
    }

    /// Whether a caller must materialize a context for this call. The selection itself remains on
    /// the call: inherited calls read the caller's incoming context, while explicit calls evaluate
    /// the selected address.
    ///
    /// # Errors
    ///
    /// Rejects a direct target outside the completed machine function domain.
    pub fn call_requires_context(
        &self,
        call: &MachineCall,
    ) -> Result<bool, MachineAllocationError> {
        self.target_requires_context(call.target())
    }
}

fn propagate_pack_callbacks(
    functions: &MachineTable<MachineFunctionId, MachineFunction>,
    previous: &[MachineAllocationRequirement],
    requirements: &mut [MachineAllocationRequirement],
) -> Result<(), MachineAllocationError> {
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
                .ok_or(MachineAllocationError::UnknownPack {
                    function: function_id,
                    pack,
                })?;
            if pack_requires_context(pack, previous)? {
                mark_incoming(requirements, *target)?;
            }
        }
    }
    Ok(())
}

fn propagate_function_bodies(
    functions: &MachineTable<MachineFunctionId, MachineFunction>,
    previous: &[MachineAllocationRequirement],
    requirements: &mut [MachineAllocationRequirement],
) -> Result<(), MachineAllocationError> {
    for (function_id, function) in functions.iter() {
        if !matches!(function.kind(), MachineFunctionKind::Callable(_)) {
            continue;
        }
        for (_, operation) in function.body().operations() {
            if operation_requires_context(operation.kind(), previous)? {
                mark_incoming(requirements, function_id)?;
                break;
            }
        }
    }
    Ok(())
}

fn operation_requires_context(
    operation: &MachineOperationKind,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    match operation {
        MachineOperationKind::Call(call) => {
            let target = target_requires_context(call.target(), requirements)?;
            Ok(target && call.allocation() == MachineCallAllocation::Inherit)
        }
        MachineOperationKind::InvokeDrop { target, .. } => {
            function_requires_incoming(requirements, *target)
        }
        _ => Ok(false),
    }
}

fn pack_requires_context(
    pack: &MachinePack,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    for segment in pack.segments() {
        match segment {
            MachinePackSegment::Value { destruction, .. } => {
                if let Some(plan) = destruction
                    && destruction_requires_context(plan, requirements)?
                {
                    return Ok(true);
                }
            }
            MachinePackSegment::Spread(spread) => {
                if target_requires_context(spread.next().target(), requirements)? {
                    return Ok(true);
                }
                if let Some(plan) = spread.destruction()
                    && destruction_requires_context(plan, requirements)?
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn destruction_requires_context(
    plan: &MachineDestructionPlan,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    match plan.kind() {
        MachineDestructionKind::Struct { drop, fields } => {
            Ok(optional_function_requires(drop.as_ref(), requirements)?
                || any_plan(
                    fields.iter().map(MachineDestructionField::plan),
                    requirements,
                )?)
        }
        MachineDestructionKind::Enum { drop, variants, .. } => {
            if optional_function_requires(drop.as_ref(), requirements)? {
                return Ok(true);
            }
            for plan in variants
                .iter()
                .flat_map(MachineDestructionVariant::payload)
                .map(MachineDestructionPayload::plan)
            {
                if destruction_requires_context(plan, requirements)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        MachineDestructionKind::FixedArray { element, .. }
        | MachineDestructionKind::Outcome {
            payload: element, ..
        }
        | MachineDestructionKind::Opaque(element) => {
            destruction_requires_context(element, requirements)
        }
        MachineDestructionKind::Closure(captures) => any_plan(
            captures.iter().map(MachineDestructionCapture::plan),
            requirements,
        ),
    }
}

fn any_plan<'plan>(
    plans: impl Iterator<Item = &'plan MachineDestructionPlan>,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    for plan in plans {
        if destruction_requires_context(plan, requirements)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn optional_function_requires(
    function: Option<&MachineFunctionId>,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    function.map_or(Ok(false), |function| {
        function_requires_incoming(requirements, *function)
    })
}

fn target_requires_context(
    target: &MachineCallTarget,
    requirements: &[MachineAllocationRequirement],
) -> Result<bool, MachineAllocationError> {
    match target {
        MachineCallTarget::Direct(function) => function_requires_incoming(requirements, *function),
        MachineCallTarget::Primitive(primitive) => Ok(matches!(
            primitive.role(),
            PrimitiveRole::CurrentAllocatorState | PrimitiveRole::CurrentAllocatorKind
        )),
    }
}

fn function_requires_incoming(
    requirements: &[MachineAllocationRequirement],
    function: MachineFunctionId,
) -> Result<bool, MachineAllocationError> {
    requirements
        .get(function.index())
        .copied()
        .map(|requirement| requirement == MachineAllocationRequirement::Incoming)
        .ok_or(MachineAllocationError::UnknownFunction(function))
}

fn mark_incoming(
    requirements: &mut [MachineAllocationRequirement],
    function: MachineFunctionId,
) -> Result<(), MachineAllocationError> {
    let requirement = requirements
        .get_mut(function.index())
        .ok_or(MachineAllocationError::UnknownFunction(function))?;
    if *requirement == MachineAllocationRequirement::None {
        *requirement = MachineAllocationRequirement::Incoming;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAllocationError {
    UnknownFunction(MachineFunctionId),
    UnknownPack {
        function: MachineFunctionId,
        pack: MachinePackId,
    },
}

impl fmt::Display for MachineAllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid machine allocation-context graph: {self:?}"
        )
    }
}

impl std::error::Error for MachineAllocationError {}
