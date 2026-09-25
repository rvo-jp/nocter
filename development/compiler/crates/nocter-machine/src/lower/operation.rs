use nocter_mir::{MirBinaryOperation, MirConstant, MirOperationKind, MirUnaryOperation};
use nocter_model::MirOperationId;

use super::MachineProgramError;
use super::aggregate::lower_aggregate;
use super::body::BodyIdentities;
use super::call::lower_call;
use super::context::ProgramLoweringContext;
use crate::{
    MachineBinaryOperation, MachineConstant, MachineOperation, MachineOperationKind,
    MachineUnaryOperation,
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
    let OperationContext { program, ids, .. } = context;
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
        MirOperationKind::Unary {
            operation,
            operand,
            check,
        } => MachineOperationKind::Unary {
            operation: lower_unary(*operation),
            operand: ids.value(*operand)?,
            check: lower_arithmetic_check(*check),
        },
        MirOperationKind::Binary {
            operation,
            left,
            right,
            check,
        } => MachineOperationKind::Binary {
            operation: lower_binary(*operation),
            left: ids.value(*left)?,
            right: ids.value(*right)?,
            check: lower_arithmetic_check(*check),
        },
        MirOperationKind::NumericConversion { operand } => {
            MachineOperationKind::NumericConversion {
                operand: ids.value(*operand)?,
            }
        }
        MirOperationKind::Aggregate(aggregate) => MachineOperationKind::Aggregate(
            lower_aggregate_operation(operation, value, aggregate, context)?,
        ),
        MirOperationKind::EraseCallable(erasure) => {
            lower_callable_erasure(operation, erasure, context)?
        }
        MirOperationKind::InvokeDrop {
            body,
            place,
            allocation,
        } => lower_drop_invocation(*body, *place, *allocation, context)?,
        MirOperationKind::ReportError { place } => MachineOperationKind::ReportError {
            place: ids.address(*place)?,
        },
        MirOperationKind::ReleaseError { place } => MachineOperationKind::ReleaseError {
            place: ids.address(*place)?,
        },
        MirOperationKind::ReleaseComputation { place } => {
            MachineOperationKind::ReleaseComputation {
                place: ids.address(*place)?,
            }
        }
        MirOperationKind::ReleaseErasedCallable { place } => {
            MachineOperationKind::ReleaseErasedCallable {
                place: ids.address(*place)?,
            }
        }
        MirOperationKind::DriveComputation {
            computation,
            destination,
        } => MachineOperationKind::DriveComputation {
            computation: ids.address(*computation)?,
            destination: destination
                .map(|destination| ids.address(destination))
                .transpose()?,
        },
        MirOperationKind::CreateRegion { parent, region } => MachineOperationKind::CreateRegion {
            parent: ids.value(*parent)?,
            region: ids.stack(*region)?,
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

const fn lower_arithmetic_check(
    check: nocter_mir::MirArithmeticCheck,
) -> crate::MachineArithmeticCheck {
    match check {
        nocter_mir::MirArithmeticCheck::NotRequired => crate::MachineArithmeticCheck::NotRequired,
        nocter_mir::MirArithmeticCheck::InternalInvariant => {
            crate::MachineArithmeticCheck::InternalInvariant
        }
        nocter_mir::MirArithmeticCheck::Required => crate::MachineArithmeticCheck::Required,
        nocter_mir::MirArithmeticCheck::ProvenSafe => crate::MachineArithmeticCheck::ProvenSafe,
        nocter_mir::MirArithmeticCheck::ProvenTrap => crate::MachineArithmeticCheck::ProvenTrap,
    }
}

fn lower_callable_erasure(
    operation: MirOperationId,
    erasure: &nocter_mir::MirErasedCallable,
    context: OperationContext<'_>,
) -> Result<MachineOperationKind, MachineProgramError> {
    let program = context.program;
    let ids = context.ids;
    let invoke = if erasure.capability() == nocter_model::CallableCapability::Owned {
        let adapter = program.erased_adapters.at(ids.owner(), operation).ok_or(
            MachineProgramError::InvalidErasedAdapter(ids.owner(), operation),
        )?;
        program
            .functions
            .for_erased_adapter(adapter)
            .ok_or(MachineProgramError::MissingErasedAdapter(adapter))?
    } else {
        program
            .functions
            .for_item(erasure.body())
            .ok_or(MachineProgramError::MissingItemFunction(erasure.body()))?
    };
    let destroy = program
        .destructions
        .erased_environment(ids.owner(), operation)
        .map(|destruction| {
            program
                .functions
                .for_destruction(destruction)
                .ok_or(MachineProgramError::MissingDestruction(destruction))
        })
        .transpose()?;
    Ok(MachineOperationKind::EraseCallable(
        crate::MachineErasedCallable::new(
            ids.value(erasure.environment())?,
            erasure.environment_ty(),
            invoke,
            destroy,
        ),
    ))
}

fn lower_drop_invocation(
    body: nocter_model::ExecutableItemId,
    place: nocter_model::MirPlaceId,
    allocation: nocter_mir::MirCallAllocation,
    context: OperationContext<'_>,
) -> Result<MachineOperationKind, MachineProgramError> {
    let ids = context.ids;
    Ok(MachineOperationKind::InvokeDrop {
        target: context
            .program
            .functions
            .for_item(body)
            .ok_or(MachineProgramError::MissingItemFunction(body))?,
        place: ids.address(place)?,
        allocation: match allocation {
            nocter_mir::MirCallAllocation::Inherit => crate::MachineCallAllocation::Inherit,
            nocter_mir::MirCallAllocation::Region(region) => {
                crate::MachineCallAllocation::Lexical(ids.stack(region)?)
            }
            nocter_mir::MirCallAllocation::Explicit(place) => {
                crate::MachineCallAllocation::Explicit(ids.address(place)?)
            }
        },
    })
}

fn lower_aggregate_operation(
    operation: MirOperationId,
    value: &nocter_mir::MirOperation,
    aggregate: &nocter_mir::MirAggregate,
    context: OperationContext<'_>,
) -> Result<crate::MachineAggregate, MachineProgramError> {
    let result = value
        .result()
        .ok_or(MachineProgramError::MissingOperationResult {
            owner: context.ids.owner(),
            operation,
        })?;
    let ty = context
        .body
        .values()
        .get(result)
        .copied()
        .ok_or(MachineProgramError::MissingOperationResult {
            owner: context.ids.owner(),
            operation,
        })?
        .ty();
    lower_aggregate(
        operation,
        aggregate,
        ty,
        context.program.layouts,
        context.ids,
    )
}

fn lower_constant(
    data: &crate::data::MachineDataPlan,
    constant: &MirConstant,
) -> Result<MachineConstant, MachineProgramError> {
    match constant {
        MirConstant::Bool(value) => Ok(MachineConstant::Bool(*value)),
        MirConstant::Character(value) => Ok(MachineConstant::Character(*value)),
        MirConstant::Float32(bits) => Ok(MachineConstant::Float32(*bits)),
        MirConstant::Float64(bits) => Ok(MachineConstant::Float64(*bits)),
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
