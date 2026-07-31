use super::*;

pub(in crate::ir::lower) fn lower_void_expression_statement(
    expression: &Expr,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    match expression {
        Expr::Call(call) => {
            if primitive_copy_str_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_copy_str_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_copy_ptr_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_copy_ptr_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_store_u8_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_store_u8_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_store_value_to_ptr_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_store_value_to_ptr_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }
            if primitive_close_fd_raw_call(call, context) {
                let mut temporaries = TemporaryAllocator::new(context)?;
                return lower_close_fd_raw_primitive_call(call, context, &mut temporaries)
                    .map(Some);
            }

            let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
                return Ok(None);
            };

            let mut temporaries = TemporaryAllocator::new(context)?;
            match context.call_return_type(&target) {
                Some(Type::Void) => lower_void_normal_call(call, context, &mut temporaries),
                Some(Type::I32) => {
                    let destination = temporaries.next_i32()?;
                    lower_i32_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::U8) => {
                    let destination = temporaries.next_u8()?;
                    lower_u8_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Usize) => {
                    let destination = temporaries.next_usize()?;
                    lower_usize_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Bool) => {
                    let destination = temporaries.next_bool()?;
                    lower_bool_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Str) => {
                    let destination = temporaries.next_str()?;
                    lower_str_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Slice { .. }) => {
                    let destination = temporaries.next_slice()?;
                    lower_slice_normal_call(call, destination, context, &mut temporaries)
                }
                Some(Type::Aggregate { .. } | Type::DirectAggregate { .. }) => {
                    lower_aggregate_normal_call_statement(call, context, &mut temporaries)
                }
                _ => return Ok(None),
            }
            .map(Some)
        }
        Expr::Group(group) => lower_void_expression_statement(&group.expression, context),
        Expr::Propagate(propagation) => lower_fallible_void_expression_statement(
            &propagation.expression,
            context,
            propagating_failure_mode(context)?,
        ),
        Expr::Force(force) => lower_fallible_void_expression_statement(
            &force.expression,
            context,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_fallible_void_expression_statement(
            &catch.expression,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                discarded_fallible_statement_reserved_abi_words(&catch.expression, context)
                    .unwrap_or(0),
            )?,
        ),
        Expr::StructLiteral(literal) => {
            lower_aggregate_struct_literal_statement(literal, context).map(Some)
        }
        _ => Ok(None),
    }
}

pub(in crate::ir::lower) fn lower_catch_failure_mode(
    catch: &CatchExpr,
    context: &LoweringContext,
    reserved_abi_words: usize,
) -> Result<FallibleFailureMode, Vec<Diagnostic>> {
    let mut catch_context = context.with_reserved_local_abi_words(reserved_abi_words);
    let (code, message) = catch_context.define_error_local(catch.error_name.clone())?;
    let instructions = lower_catch_block(&catch.catch_block, &mut catch_context)?;

    Ok(FallibleFailureMode::Catch {
        code,
        message,
        instructions,
    })
}
