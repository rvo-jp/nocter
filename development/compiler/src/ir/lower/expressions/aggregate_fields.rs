use super::*;

pub(in crate::ir::lower) fn push_store_str_view_to_aggregate_field(
    instructions: &mut Vec<Instruction>,
    destination: AggregateLocation,
    offset: u32,
    value: StrValue,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let temporary = temporaries.next_str()?;
    let StrLocation::Local(index) = temporary else {
        unreachable!("temporary str locations are local pairs");
    };
    let len_index = index.checked_add(1).ok_or_else(&unsupported_diagnostic)?;
    let len_offset = offset.checked_add(8).ok_or_else(unsupported_diagnostic)?;

    instructions.push(Instruction::SetStr {
        destination: temporary,
        value,
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset,
        value: UsizeValue::Location(UsizeLocation::Local(index)),
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset: len_offset,
        value: UsizeValue::Location(UsizeLocation::Local(len_index)),
    });
    Ok(())
}

pub(in crate::ir::lower) fn push_store_slice_view_to_aggregate_field(
    instructions: &mut Vec<Instruction>,
    destination: AggregateLocation,
    offset: u32,
    value: SliceValue,
    temporaries: &mut TemporaryAllocator,
    unsupported_diagnostic: impl Fn() -> Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    let temporary = temporaries.next_slice()?;
    let SliceLocation::Local(index) = temporary else {
        unreachable!("temporary slice locations are local pairs");
    };
    let len_index = index.checked_add(1).ok_or_else(&unsupported_diagnostic)?;
    let len_offset = offset.checked_add(8).ok_or_else(unsupported_diagnostic)?;

    instructions.push(Instruction::SetSlice {
        destination: temporary,
        value,
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset,
        value: UsizeValue::Location(UsizeLocation::Local(index)),
    });
    instructions.push(Instruction::StoreAggregateUsize {
        destination,
        offset: len_offset,
        value: UsizeValue::Location(UsizeLocation::Local(len_index)),
    });
    Ok(())
}

pub(in crate::ir::lower) struct LoweredAggregateFieldAccess {
    pub(in crate::ir::lower) instructions: Vec<Instruction>,
    pub(in crate::ir::lower) source: AggregateLocation,
    pub(in crate::ir::lower) offset: u32,
    pub(in crate::ir::lower) kind: AggregateFieldKind,
    pub(in crate::ir::lower) is_readwrite: bool,
    pub(in crate::ir::lower) is_copy: bool,
}

pub(in crate::ir::lower) fn lower_aggregate_member_field_access(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredAggregateFieldAccess>, Vec<Diagnostic>> {
    let Some(access) = aggregate_member_access(expression, context)? else {
        return Ok(None);
    };
    match access.root {
        AggregateMemberRoot::Identifier(identifier_name) => Ok(context
            .aggregate_field(identifier_name, &access.field_path)
            .map(|field| LoweredAggregateFieldAccess {
                instructions: Vec::new(),
                source: field.source,
                offset: field.offset,
                kind: field.kind,
                is_readwrite: field.is_readwrite,
                is_copy: field.is_copy,
            })),
        AggregateMemberRoot::Call(call) => {
            lower_aggregate_call_member_field_access(call, &access.field_path, context, temporaries)
        }
        AggregateMemberRoot::FallibleCall(call, failure_mode) => {
            lower_aggregate_fallible_call_member_field_access(
                call,
                &access.field_path,
                context,
                temporaries,
                failure_mode,
            )
        }
        AggregateMemberRoot::OptionalCall(otherwise) => {
            lower_aggregate_optional_otherwise_member_field_access(
                otherwise,
                &access.field_path,
                context,
                temporaries,
            )
        }
    }
}

pub(in crate::ir::lower) fn aggregate_member_field_kind_from_member(
    member: &crate::ast::MemberExpr,
    context: &LoweringContext,
) -> Result<Option<AggregateFieldKind>, Vec<Diagnostic>> {
    let Some((root, mut fields)) = aggregate_member_root_and_path(&member.object, context)? else {
        return Ok(None);
    };
    fields.push(member.member.as_str());
    let field_path = fields.join(".");
    Ok(match root {
        AggregateMemberRoot::Identifier(identifier_name) => context
            .aggregate_field(identifier_name, &field_path)
            .map(|field| field.kind),
        AggregateMemberRoot::Call(call) => {
            aggregate_call_member_field_kind(call, &field_path, context)
        }
        AggregateMemberRoot::FallibleCall(call, _) => {
            aggregate_outcome_call_member_field_kind(call, &field_path, context)
        }
        AggregateMemberRoot::OptionalCall(otherwise) => {
            aggregate_optional_otherwise_member_field_kind(otherwise, &field_path, context)
        }
    })
}

pub(in crate::ir::lower) fn aggregate_call_field(
    call: &CallExpr,
    member_name: &str,
    context: &LoweringContext,
) -> Option<super::super::context::AggregateField> {
    let (root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    aggregate_fields_from_type_expr(&return_type, root_source, resolved)?
        .into_iter()
        .find(|field| field.name == member_name)
}
