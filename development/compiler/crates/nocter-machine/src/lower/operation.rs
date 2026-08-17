use std::collections::BTreeMap;

use nocter_mir::{
    MirBinaryOperation, MirCallAllocation, MirCallTarget, MirConstant, MirOperationKind,
    MirUnaryOperation,
};
use nocter_model::{ExecutableItemId, MirOperationId};

use super::aggregate::lower_aggregate;
use super::body::BodyIdentities;
use super::{MachineProgramError, MachineUnsupportedOperation, unsupported};
use crate::{
    MachineBinaryOperation, MachineCallAllocation, MachineConstant, MachineDataTable,
    MachineDirectCall, MachineFunctionId, MachineLayoutStore, MachineOperation,
    MachineOperationKind, MachineUnaryOperation,
};

pub(super) fn lower_operations(
    body: &nocter_mir::MirBody,
    layouts: &MachineLayoutStore,
    data: &MachineDataTable,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ids: &BodyIdentities,
) -> Result<Vec<MachineOperation>, MachineProgramError> {
    body.operations()
        .iter()
        .map(|(operation, value)| {
            lower_operation(operation, value, body, layouts, data, functions, ids)
        })
        .collect()
}

fn lower_operation(
    operation: MirOperationId,
    value: &nocter_mir::MirOperation,
    body: &nocter_mir::MirBody,
    layouts: &MachineLayoutStore,
    data: &MachineDataTable,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ids: &BodyIdentities,
) -> Result<MachineOperation, MachineProgramError> {
    let result = value.result().map(|result| ids.value(result)).transpose()?;
    let kind = match value.kind() {
        MirOperationKind::Constant(constant) => {
            MachineOperationKind::Constant(lower_constant(data, constant)?)
        }
        MirOperationKind::Read { place, .. } => MachineOperationKind::Load {
            source: ids.address(*place)?,
        },
        MirOperationKind::Borrow { place, .. } => MachineOperationKind::AddressOf {
            source: ids.address(*place)?,
        },
        MirOperationKind::Store { destination, value }
        | MirOperationKind::Initialize { destination, value } => MachineOperationKind::Store {
            destination: ids.address(*destination)?,
            value: ids.value(*value)?,
        },
        MirOperationKind::SetDropFlag { flag, initialized } => MachineOperationKind::SetDropFlag {
            flag: ids.drop_flag(*flag)?,
            initialized: *initialized,
        },
        MirOperationKind::Unary { operation, operand } => MachineOperationKind::Unary {
            operation: lower_unary(*operation),
            operand: ids.value(*operand)?,
        },
        MirOperationKind::Binary {
            operation,
            left,
            right,
        } => MachineOperationKind::Binary {
            operation: lower_binary(*operation),
            left: ids.value(*left)?,
            right: ids.value(*right)?,
        },
        MirOperationKind::IntegerConversion { operand } => {
            MachineOperationKind::IntegerConversion {
                operand: ids.value(*operand)?,
            }
        }
        MirOperationKind::Aggregate(aggregate) => {
            let result = value
                .result()
                .ok_or(MachineProgramError::MissingOperationResult {
                    owner: ids.owner(),
                    operation,
                })?;
            let ty = body
                .values()
                .get(result)
                .copied()
                .ok_or(MachineProgramError::MissingOperationResult {
                    owner: ids.owner(),
                    operation,
                })?
                .ty();
            MachineOperationKind::Aggregate(lower_aggregate(
                operation, aggregate, ty, layouts, ids,
            )?)
        }
        MirOperationKind::InvokeDrop { body, place } => MachineOperationKind::InvokeDrop {
            target: functions
                .get(body)
                .copied()
                .ok_or(MachineProgramError::MissingItemFunction(*body))?,
            place: ids.address(*place)?,
        },
        MirOperationKind::ReportError { error } => MachineOperationKind::ReportError {
            error: ids.value(*error)?,
        },
        MirOperationKind::CreateRegion { parent } => MachineOperationKind::CreateRegion {
            parent: ids.value(*parent)?,
        },
        MirOperationKind::ReleaseRegion { region } => MachineOperationKind::ReleaseRegion {
            region: ids.stack(*region)?,
        },
        MirOperationKind::Call(call) => lower_direct_call(operation, call, functions, ids)?,
        kind => {
            return Err(unsupported(
                ids.owner(),
                operation,
                MachineUnsupportedOperation::from(kind),
            ));
        }
    };
    Ok(MachineOperation::new(kind, result))
}

fn lower_direct_call(
    operation: MirOperationId,
    call: &nocter_mir::MirCall,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
    ids: &BodyIdentities,
) -> Result<MachineOperationKind, MachineProgramError> {
    if call.pack().is_some() {
        return Err(unsupported(
            ids.owner(),
            operation,
            MachineUnsupportedOperation::PackedCall,
        ));
    }
    let MirCallTarget::Direct(target) = call.target() else {
        let kind = match call.target() {
            MirCallTarget::StandardPrimitive { .. } => {
                MachineUnsupportedOperation::StandardPrimitiveCall
            }
            MirCallTarget::Structural(_) => MachineUnsupportedOperation::StructuralCall,
            MirCallTarget::Direct(_) => unreachable!(),
        };
        return Err(unsupported(ids.owner(), operation, kind));
    };
    let function = functions
        .get(target)
        .copied()
        .ok_or(MachineProgramError::MissingItemFunction(*target))?;
    let arguments = call
        .arguments()
        .iter()
        .map(|argument| ids.value(*argument))
        .collect::<Result<Vec<_>, _>>()?;
    let allocation = match call.allocation() {
        MirCallAllocation::Inherit => MachineCallAllocation::Inherit,
        MirCallAllocation::Explicit(place) => MachineCallAllocation::Explicit(ids.address(place)?),
    };
    Ok(MachineOperationKind::DirectCall(MachineDirectCall::new(
        function, arguments, allocation,
    )))
}

fn lower_constant(
    data: &MachineDataTable,
    constant: &MirConstant,
) -> Result<MachineConstant, MachineProgramError> {
    match constant {
        MirConstant::Bool(value) => Ok(MachineConstant::Bool(*value)),
        MirConstant::Integer(value) => Ok(MachineConstant::Integer(*value)),
        MirConstant::Text(text) => data
            .text(text)
            .map(MachineConstant::Text)
            .ok_or_else(|| MachineProgramError::MissingStaticText(text.clone())),
    }
}

const fn lower_unary(operation: MirUnaryOperation) -> MachineUnaryOperation {
    match operation {
        MirUnaryOperation::LogicalNot => MachineUnaryOperation::LogicalNot,
        MirUnaryOperation::Negate => MachineUnaryOperation::Negate,
    }
}

const fn lower_binary(operation: MirBinaryOperation) -> MachineBinaryOperation {
    match operation {
        MirBinaryOperation::Add => MachineBinaryOperation::Add,
        MirBinaryOperation::Subtract => MachineBinaryOperation::Subtract,
        MirBinaryOperation::Multiply => MachineBinaryOperation::Multiply,
        MirBinaryOperation::Divide => MachineBinaryOperation::Divide,
        MirBinaryOperation::Remainder => MachineBinaryOperation::Remainder,
        MirBinaryOperation::ShiftLeft => MachineBinaryOperation::ShiftLeft,
        MirBinaryOperation::ShiftRightSigned => MachineBinaryOperation::ShiftRightSigned,
        MirBinaryOperation::ShiftRightUnsigned => MachineBinaryOperation::ShiftRightUnsigned,
        MirBinaryOperation::Equal => MachineBinaryOperation::Equal,
        MirBinaryOperation::Less => MachineBinaryOperation::Less,
    }
}
