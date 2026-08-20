use nocter_mir::{MirBinaryOperation, MirConstant, MirOperationKind, MirUnaryOperation};
use nocter_model::MirOperationId;

use super::MachineProgramError;
use super::aggregate::lower_aggregate;
use super::body::BodyIdentities;
use super::call::lower_call;
use super::context::ProgramLoweringContext;
use crate::{
    MachineBinaryOperation, MachineConstant, MachineDataTable, MachineOperation,
    MachineOperationKind, MachineUnaryOperation,
};

pub(super) fn lower_operations(
    body: &nocter_mir::MirBody,
    program: ProgramLoweringContext<'_>,
    ids: &BodyIdentities,
) -> Result<Vec<MachineOperation>, MachineProgramError> {
    let context = OperationContext { body, program, ids };
    body.operations()
        .iter()
        .map(|(operation, value)| lower_operation(operation, value, context))
        .collect()
}

#[derive(Clone, Copy)]
struct OperationContext<'a> {
    body: &'a nocter_mir::MirBody,
    program: ProgramLoweringContext<'a>,
    ids: &'a BodyIdentities,
}

fn lower_operation(
    operation: MirOperationId,
    value: &nocter_mir::MirOperation,
    context: OperationContext<'_>,
) -> Result<MachineOperation, MachineProgramError> {
    let OperationContext { body, program, ids } = context;
    let result = value.result().map(|result| ids.value(result)).transpose()?;
    let kind = match value.kind() {
        MirOperationKind::Constant(constant) => {
            MachineOperationKind::Constant(lower_constant(program.data, constant)?)
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
                operation,
                aggregate,
                ty,
                program.layouts,
                ids,
            )?)
        }
        MirOperationKind::InvokeDrop { body, place } => MachineOperationKind::InvokeDrop {
            target: program
                .functions
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
        MirOperationKind::Call(call) => lower_call(operation, call, program, ids)?,
        MirOperationKind::PackLength => MachineOperationKind::PackLength,
        MirOperationKind::PackNext => MachineOperationKind::PackNext,
        MirOperationKind::DestroyPack => MachineOperationKind::DestroyPack,
    };
    Ok(MachineOperation::new(kind, result))
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
