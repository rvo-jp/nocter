use nocter_mir::MirStructuralCall;
use nocter_model::{BorrowCapability, MirOperationId, TypeId};
use nocter_runtime_contract::{RuntimePrimitive, RuntimeType, RuntimeTypeTable};

use super::MachineProgramError;
use super::body::BodyIdentities;
use crate::{
    MachineComparison, MachineComparisonOperation, MachineComparisonRepresentation,
    MachineIndexBorrow, MachineIndexDomain, MachineLayoutKind, MachineLayoutPlan,
    MachineOperationKind, MachineScalar, MachineStructuralError,
};

pub(super) fn lower_structural(
    operation: MirOperationId,
    target: &MirStructuralCall,
    arguments: &[nocter_model::MirValueId],
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutPlan,
    ids: &BodyIdentities,
) -> Result<MachineOperationKind, MachineProgramError> {
    let context = StructuralContext {
        operation,
        types,
        layouts,
        ids,
    };
    match target {
        MirStructuralCall::Equality { subject, operand } => lower_comparison(
            *subject,
            *operand,
            arguments,
            MachineComparisonOperation::Equal,
            context,
        ),
        MirStructuralCall::Ordering { subject, operand } => lower_comparison(
            *subject,
            *operand,
            arguments,
            MachineComparisonOperation::Less,
            context,
        ),
        MirStructuralCall::Index {
            capability,
            container,
            receiver,
            index,
            result,
        } => lower_index(
            *capability,
            *container,
            *receiver,
            *index,
            *result,
            arguments,
            context,
        ),
        MirStructuralCall::BorrowWeakening { source, target } => {
            lower_borrow_weakening(*source, *target, arguments, context)
        }
    }
}

#[derive(Clone, Copy)]
struct StructuralContext<'a> {
    operation: MirOperationId,
    types: &'a RuntimeTypeTable,
    layouts: &'a MachineLayoutPlan,
    ids: &'a BodyIdentities,
}

impl StructuralContext<'_> {
    const fn error(self, error: MachineStructuralError) -> MachineProgramError {
        MachineProgramError::Structural {
            owner: self.ids.owner(),
            operation: self.operation,
            error,
        }
    }
}

fn lower_comparison(
    subject: TypeId,
    operand: TypeId,
    arguments: &[nocter_model::MirValueId],
    comparison: MachineComparisonOperation,
    context: StructuralContext<'_>,
) -> Result<MachineOperationKind, MachineProgramError> {
    let [left, right] = arguments else {
        return Err(context.error(MachineStructuralError::InvalidSignature));
    };
    if !matches!(
        context.types.get(operand),
        Some(RuntimeType::Borrow {
            capability: BorrowCapability::Readonly,
            referent,
        }) if *referent == subject
    ) || !matches!(
        context.layouts.get(operand).map(crate::MachineLayout::kind),
        Some(MachineLayoutKind::Pointer)
    ) {
        return Err(context.error(MachineStructuralError::InvalidRepresentation));
    }
    let representation = match context.layouts.get(subject).map(crate::MachineLayout::kind) {
        Some(MachineLayoutKind::Scalar(scalar))
            if comparison == MachineComparisonOperation::Equal
                || matches!(scalar, MachineScalar::Integer { .. }) =>
        {
            MachineComparisonRepresentation::Scalar(*scalar)
        }
        Some(MachineLayoutKind::Enum {
            tag_offset,
            variants,
            ..
        }) if comparison == MachineComparisonOperation::Equal
            && variants.iter().all(|variant| variant.payload().is_empty()) =>
        {
            MachineComparisonRepresentation::Tag {
                offset: *tag_offset,
            }
        }
        _ => {
            return Err(context.error(MachineStructuralError::InvalidRepresentation));
        }
    };
    Ok(MachineOperationKind::Comparison(MachineComparison::new(
        comparison,
        representation,
        context.ids.value(*left)?,
        context.ids.value(*right)?,
    )))
}

fn lower_index(
    capability: BorrowCapability,
    container: TypeId,
    receiver: TypeId,
    index: TypeId,
    result: TypeId,
    arguments: &[nocter_model::MirValueId],
    context: StructuralContext<'_>,
) -> Result<MachineOperationKind, MachineProgramError> {
    let [receiver_value, index_value] = arguments else {
        return Err(context.error(MachineStructuralError::InvalidSignature));
    };
    let valid_signature = context.types.primitive(RuntimePrimitive::Usize) == Some(index)
        && matches!(
            context.types.get(receiver),
            Some(RuntimeType::Borrow {
                capability: actual,
                referent,
            }) if *actual == capability && *referent == container
        );
    if !valid_signature
        || !matches!(
            context.layouts.get(result).map(crate::MachineLayout::kind),
            Some(MachineLayoutKind::Pointer)
        )
    {
        return Err(context.error(MachineStructuralError::InvalidSignature));
    }
    let (element, domain) = index_domain(container, receiver, context.types, context.layouts)
        .ok_or_else(|| context.error(MachineStructuralError::InvalidRepresentation))?;
    if !matches!(
        context.types.get(result),
        Some(RuntimeType::Borrow {
            capability: actual,
            referent,
        }) if *actual == capability && *referent == element
    ) {
        return Err(context.error(MachineStructuralError::InvalidSignature));
    }
    Ok(MachineOperationKind::IndexBorrow(MachineIndexBorrow::new(
        context.ids.value(*receiver_value)?,
        context.ids.value(*index_value)?,
        domain,
    )))
}

fn index_domain(
    container: TypeId,
    receiver: TypeId,
    types: &RuntimeTypeTable,
    layouts: &MachineLayoutPlan,
) -> Option<(TypeId, MachineIndexDomain)> {
    match types.get(container)? {
        RuntimeType::FixedArray { element, .. } => {
            if !matches!(layouts.get(receiver)?.kind(), MachineLayoutKind::Pointer) {
                return None;
            }
            let MachineLayoutKind::FixedArray { length, stride, .. } =
                layouts.get(container)?.kind()
            else {
                return None;
            };
            Some((
                *element,
                MachineIndexDomain::Fixed {
                    length: *length,
                    stride: *stride,
                },
            ))
        }
        RuntimeType::Slice(element) => view_index_domain(*element, receiver, layouts),
        RuntimeType::Primitive(RuntimePrimitive::Text) => view_index_domain(
            types.primitive(RuntimePrimitive::Unsigned(8))?,
            receiver,
            layouts,
        ),
        _ => None,
    }
}

fn view_index_domain(
    element: TypeId,
    receiver: TypeId,
    layouts: &MachineLayoutPlan,
) -> Option<(TypeId, MachineIndexDomain)> {
    let MachineLayoutKind::View {
        pointer_offset,
        length_offset,
    } = layouts.get(receiver)?.kind()
    else {
        return None;
    };
    let stride = layouts.get(element)?.size();
    Some((
        element,
        MachineIndexDomain::View {
            pointer_offset: *pointer_offset,
            length_offset: *length_offset,
            stride,
        },
    ))
}

fn lower_borrow_weakening(
    source: TypeId,
    target: TypeId,
    arguments: &[nocter_model::MirValueId],
    context: StructuralContext<'_>,
) -> Result<MachineOperationKind, MachineProgramError> {
    let [value] = arguments else {
        return Err(context.error(MachineStructuralError::InvalidSignature));
    };
    let valid_types = matches!(
        (context.types.get(source), context.types.get(target)),
        (
            Some(RuntimeType::Borrow {
                capability: BorrowCapability::ReadWrite,
                referent: source_referent,
            }),
            Some(RuntimeType::Borrow {
                capability: BorrowCapability::Readonly,
                referent: target_referent,
            }),
        ) if source_referent == target_referent
    );
    if !valid_types || context.layouts.get(source) != context.layouts.get(target) {
        return Err(context.error(MachineStructuralError::InvalidRepresentation));
    }
    Ok(MachineOperationKind::BorrowWeakening {
        source: context.ids.value(*value)?,
    })
}
