//! Native lowering for source-defined index operations.
//!
//! The immutable typecheck plan is the only selector. Declared operations enter the ordinary
//! borrow-returning static-call ABI here; primitive arrays and views never use this adapter.

use super::*;

pub(in crate::ir::lower) fn lower_declared_index_pointer(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<(Vec<Instruction>, UsizeLocation)>, Vec<Diagnostic>> {
    let Some(plan) = context.index_plan(expression.span) else {
        return Ok(None);
    };
    if plan.projection != crate::typecheck::TypecheckIndexProjection::Declared {
        return Ok(None);
    }
    let method = plan
        .method
        .as_ref()
        .ok_or_else(unavailable_call_target_diagnostic)?;
    let target = context
        .protocol_method_target(method)
        .ok_or_else(unavailable_call_target_diagnostic)?;
    let receiver = if matches!(plan.target_ty, crate::ast::TypeExpr::Borrow(_)) {
        expression.object.as_ref().clone()
    } else {
        Expr::Borrow(crate::ast::BorrowExpr {
            span: expression.object.span(),
            operator_span: expression.object.span(),
            is_readwrite: method.receiver_mode == crate::ast::MethodReceiverMode::ReadwriteBorrow,
            expression: expression.object.clone(),
        })
    };
    let call = crate::ir::lower::collection_for::synthetic_call(
        expression.span,
        &method.target_name,
        vec![receiver, expression.index.as_ref().clone()],
    );
    let parameter_types = vec![
        crate::ir::lower::collection_for::receiver_parameter_type(method, expression.span),
        plan.index_ty.clone(),
    ];
    let destination = temporaries.next_usize()?;
    let (mut instructions, arguments) = lower_call_arguments_with_explicit_types(
        &call,
        &target,
        &method.target_name,
        context,
        temporaries,
        Some(&parameter_types),
    )?;
    instructions.push(Instruction::CallBorrow {
        destination,
        target,
        arguments,
    });
    Ok(Some((instructions, destination)))
}

pub(super) fn lower_declared_i32_index(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredI32Value>, Vec<Diagnostic>> {
    let Some((mut instructions, pointer)) =
        lower_declared_index_pointer(expression, context, temporaries)?
    else {
        return Ok(None);
    };
    let destination = temporaries.next_i32()?;
    instructions.push(Instruction::LoadI32FromPointer {
        destination,
        pointer: UsizeValue::Location(pointer),
        offset: UsizeValue::Const(0),
    });
    Ok(Some(LoweredI32Value {
        instructions,
        value: I32Value::Location(destination),
    }))
}

pub(super) fn lower_declared_u8_index(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredU8Value>, Vec<Diagnostic>> {
    let Some((mut instructions, pointer)) =
        lower_declared_index_pointer(expression, context, temporaries)?
    else {
        return Ok(None);
    };
    let destination = temporaries.next_u8()?;
    instructions.push(Instruction::LoadU8FromPointer {
        destination,
        pointer: UsizeValue::Location(pointer),
        offset: UsizeValue::Const(0),
    });
    Ok(Some(LoweredU8Value {
        instructions,
        value: U8Value::Location(destination),
    }))
}

pub(super) fn lower_declared_usize_index(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredUsizeValue>, Vec<Diagnostic>> {
    let Some((mut instructions, pointer)) =
        lower_declared_index_pointer(expression, context, temporaries)?
    else {
        return Ok(None);
    };
    let destination = temporaries.next_usize()?;
    let element_type = context
        .expression_type_expr(expression.span)
        .and_then(|ty| {
            let (_, resolved) = context.resolved_calls()?;
            scalar_or_view_type_from_type_expr(&ty, resolved)
        });
    if let Some(Type::Integer(kind)) = element_type {
        instructions.push(Instruction::LoadIntegerFromPointer {
            kind,
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        });
    } else {
        instructions.push(Instruction::LoadUsizeFromPointer {
            destination,
            pointer: UsizeValue::Location(pointer),
            offset: UsizeValue::Const(0),
        });
    }
    Ok(Some(LoweredUsizeValue {
        instructions,
        value: UsizeValue::Location(destination),
    }))
}

pub(super) fn lower_declared_bool_index(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredBoolValue>, Vec<Diagnostic>> {
    let Some((mut instructions, pointer)) =
        lower_declared_index_pointer(expression, context, temporaries)?
    else {
        return Ok(None);
    };
    let destination = temporaries.next_bool()?;
    instructions.push(Instruction::LoadBoolFromPointer {
        destination,
        pointer: UsizeValue::Location(pointer),
        offset: UsizeValue::Const(0),
    });
    Ok(Some(LoweredBoolValue {
        instructions,
        value: BoolValue::Location(destination),
    }))
}

pub(super) fn lower_declared_str_index(
    expression: &IndexExpr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Option<LoweredStrValue>, Vec<Diagnostic>> {
    let Some((mut instructions, pointer)) =
        lower_declared_index_pointer(expression, context, temporaries)?
    else {
        return Ok(None);
    };
    let destination = temporaries.next_str()?;
    instructions.push(Instruction::LoadStrFromPointer {
        destination,
        pointer: UsizeValue::Location(pointer),
        offset: UsizeValue::Const(0),
    });
    Ok(Some(LoweredStrValue {
        instructions,
        value: StrValue::Location(destination),
    }))
}
