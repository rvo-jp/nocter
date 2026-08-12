use super::*;

pub(super) fn optional_success_scalar_binding_kind(
    statement: &BindingStmt,
    success_type: &Type,
    context: &LoweringContext,
) -> Result<Option<ScalarBindingKind>, Vec<Diagnostic>> {
    let Some(ty) = &statement.ty else {
        return Ok(scalar_binding_kind_from_type(success_type));
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "native lowering cannot lower annotated `otherwise` bindings without resolved type information",
        ));
    };
    Ok(
        match parameter_type_from_type_expr_with_resolver(ty, resolved, |_| Some(resolved)) {
            Some(Type::I32) => Some(ScalarBindingKind::I32),
            Some(Type::U8) => Some(ScalarBindingKind::U8),
            Some(Type::Usize) => Some(ScalarBindingKind::Usize),
            Some(Type::Bool) => Some(ScalarBindingKind::Bool),
            Some(Type::Str) => Some(ScalarBindingKind::Str),
            Some(Type::Slice { .. }) => Some(ScalarBindingKind::Slice(
                slice_type_info_from_type_expr(ty, context),
            )),
            Some(Type::Borrow {
                is_readwrite,
                inner,
            }) => Some(ScalarBindingKind::Borrow {
                is_readwrite,
                inner: *inner,
            }),
            _ => None,
        },
    )
}

pub(super) fn lower_otherwise_terminal_block(
    block: &Block,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Some(result) = &block.result {
        let mut instructions = Vec::new();
        for statement in &block.statements {
            instructions.extend(lower_otherwise_leading_statement(
                statement,
                context,
                loop_control,
            )?);
        }

        let Some(terminating_instructions) = lower_never_expression(result, context)? else {
            return Err(unsupported_binding_diagnostic(
                "native lowering can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
            ));
        };
        instructions.extend(terminating_instructions);
        return Ok(instructions);
    }

    let Some((terminal, leading)) = block.statements.split_last() else {
        return Err(unsupported_binding_diagnostic(
            "native lowering cannot lower empty `otherwise` fallback blocks",
        ));
    };

    let mut instructions = Vec::new();
    for statement in leading {
        instructions.extend(lower_otherwise_leading_statement(
            statement,
            context,
            loop_control,
        )?);
    }

    match terminal {
        Stmt::Return(statement) => {
            instructions.extend(lower_return_statement_with_scope_drops(
                statement, context, "E8008",
            )?);
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_expression(&statement.expression, context)?
            else {
                return Err(unsupported_binding_diagnostic(
                    "native lowering can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
                ));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        Stmt::Break(_) => {
            instructions.extend(lower_otherwise_loop_control_statement(
                Instruction::Break,
                context,
                loop_control,
            )?);
            Ok(instructions)
        }
        Stmt::Continue(_) => {
            instructions.extend(lower_otherwise_loop_control_statement(
                Instruction::Continue,
                context,
                loop_control,
            )?);
            Ok(instructions)
        }
        _ => Err(unsupported_binding_diagnostic(
            "native lowering can only lower `otherwise` fallback blocks ending in `return`, `break`, `continue`, or a `never` expression",
        )),
    }
}

pub(super) fn lower_otherwise_leading_statement(
    statement: &Stmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match statement {
        Stmt::Import(_) | Stmt::FromImport(_) => Ok(Vec::new()),
        Stmt::Binding(statement) => lower_local_binding_with_loop_control(
            statement,
            context,
            loop_control,
        ),
        Stmt::Assignment(statement) => lower_assignment(statement, context),
        Stmt::Drop(statement) => lower_drop_statement(statement, context),
        Stmt::Expression(statement) => {
            lower_void_expression_statement(&statement.expression, context)?.ok_or_else(|| {
                unsupported_binding_diagnostic(
                    "native lowering can only lower `otherwise` leading expression statements that make effect-only calls",
                )
            })
        }
        _ => Err(unsupported_binding_diagnostic(
            "native lowering cannot lower this statement inside `otherwise` fallback blocks",
        )),
    }
}

pub(super) fn lower_otherwise_loop_control_statement(
    instruction: Instruction,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(loop_control) = loop_control else {
        return Err(unsupported_binding_diagnostic(
            "native lowering can only lower `break` and `continue` inside `otherwise` fallback blocks when the binding is inside a nonterminal loop",
        ));
    };

    let mut instructions =
        lower_scope_end_drops_for_locals_since(context, loop_control.scope_mark.locals)?;
    instructions.extend(context.region_cleanup_instructions_since(loop_control.scope_mark.regions));
    if matches!(instruction, Instruction::Continue) {
        instructions.extend(loop_control.continue_instructions.iter().cloned());
    }
    instructions.push(instruction);
    Ok(instructions)
}

pub(in crate::ir::lower) fn lower_otherwise_recover_or_handle_failure_mode<F>(
    fallback: &Block,
    context: &LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
    mut lower_result: F,
    unsupported_message: &'static str,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
{
    lower_fallback_block_with_result_mode(
        fallback,
        context,
        loop_control,
        |result, context| lower_result(result, context).map(LoweredFallbackResult::Continue),
        unsupported_message,
    )
}

#[derive(Debug)]
pub(in crate::ir::lower) enum LoweredFallbackResult {
    Continue(Vec<Instruction>),
    Terminate(Vec<Instruction>),
}

pub(in crate::ir::lower) fn lower_fallback_block_with_result_mode<F>(
    fallback: &Block,
    context: &LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
    mut lower_result: F,
    unsupported_message: &'static str,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<LoweredFallbackResult, Vec<Diagnostic>>,
{
    let mut fallback_context = context.clone();
    let local_mark = fallback_context.local_mark();

    let Some(result) = &fallback.result else {
        let instructions =
            lower_otherwise_terminal_block(fallback, &mut fallback_context, loop_control)?;
        return Ok(OutcomeFailureMode::Handle { instructions });
    };

    let mut leading_instructions = Vec::new();
    for statement in &fallback.statements {
        leading_instructions.extend(lower_otherwise_leading_statement(
            statement,
            &mut fallback_context,
            loop_control,
        )?);
    }
    if let Some(terminating_instructions) = lower_never_expression(result, &mut fallback_context)? {
        leading_instructions.extend(terminating_instructions);
        return Ok(OutcomeFailureMode::Handle {
            instructions: leading_instructions,
        });
    }

    let result = lower_result(result, &fallback_context)
        .map_err(|_| unsupported_binding_diagnostic(unsupported_message))?;
    match result {
        LoweredFallbackResult::Continue(instructions) => {
            leading_instructions.extend(instructions);
            leading_instructions.extend(lower_scope_end_drops_for_locals_since(
                &mut fallback_context,
                local_mark,
            )?);
            Ok(OutcomeFailureMode::Recover {
                instructions: leading_instructions,
            })
        }
        LoweredFallbackResult::Terminate(instructions) => {
            leading_instructions.extend(lower_scope_end_drops_for_locals_since(
                &mut fallback_context,
                local_mark,
            )?);
            leading_instructions.extend(instructions);
            Ok(OutcomeFailureMode::Handle {
                instructions: leading_instructions,
            })
        }
    }
}

pub(super) fn lower_otherwise_scalar_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Otherwise(otherwise) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    if let Expr::Propagate(propagate) = unwrap_group(&otherwise.value)
        && let Expr::Call(call) = unwrap_group(&propagate.expression)
        && let Some((target, _)) = context.direct_call_target_and_name(call)
        && let Some(Type::ComposedOutcome {
            outer: crate::outcomes::OutcomeLayer::Fallible,
            inner: crate::outcomes::OutcomeLayer::Optional,
            payload,
        }) = context.call_return_type(&target).cloned()
        && let Some(kind) = optional_success_scalar_binding_kind(statement, &payload, context)?
    {
        return lower_composed_otherwise_scalar_call_binding(
            statement,
            call,
            &otherwise.fallback,
            kind,
            ComposedOuterHandler::Mode(propagating_outcome_mode(&propagate.expression, context)?),
            context,
            loop_control,
        )
        .map(Some);
    }
    if let Expr::Catch(catch) = unwrap_group(&otherwise.value)
        && let Expr::Call(call) = unwrap_group(&catch.expression)
        && let Some((target, _)) = context.direct_call_target_and_name(call)
        && let Some(Type::ComposedOutcome {
            outer: crate::outcomes::OutcomeLayer::Fallible,
            inner: crate::outcomes::OutcomeLayer::Optional,
            payload,
        }) = context.call_return_type(&target).cloned()
        && let Some(kind) = optional_success_scalar_binding_kind(statement, &payload, context)?
    {
        return lower_composed_otherwise_scalar_call_binding(
            statement,
            call,
            &otherwise.fallback,
            kind,
            ComposedOuterHandler::Catch(catch),
            context,
            loop_control,
        )
        .map(Some);
    }
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "native lowering cannot lower `otherwise` bindings without resolved call information",
        ));
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Ok(None);
    }

    let Some((target, _call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Optional(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(kind) =
        optional_success_scalar_binding_kind(statement, success_type.as_ref(), context)?
    else {
        return Ok(None);
    };
    lower_otherwise_scalar_call_binding(
        statement,
        call,
        &otherwise.fallback,
        kind,
        context,
        loop_control,
    )
    .map(Some)
}

enum ComposedOuterHandler<'a> {
    Mode(OutcomeFailureMode),
    Catch(&'a CatchExpr),
}

fn lower_composed_otherwise_scalar_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    fallback: &Block,
    kind: ScalarBindingKind,
    outer_handler: ComposedOuterHandler<'_>,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expression_context = context.with_reserved_local_abi_words(kind.abi_word_count());
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;

    let (destination, inner_mode) = match &kind {
        ScalarBindingKind::I32 => {
            let destination = context.next_i32_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_i32_expression_to_location(expression, destination, context)
                },
                "IR can only lower i32 composed-outcome fallbacks that produce i32 or exit",
            )?;
            (ComposedOutcomeDestination::I32(destination), mode)
        }
        ScalarBindingKind::U8 => {
            let destination = context.next_u8_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_u8_expression_to_location(expression, destination, context)
                },
                "IR can only lower u8 composed-outcome fallbacks that produce u8 or exit",
            )?;
            (ComposedOutcomeDestination::U8(destination), mode)
        }
        ScalarBindingKind::Usize => {
            let destination = context.next_usize_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_usize_expression_to_location(expression, destination, context)
                },
                "IR can only lower usize composed-outcome fallbacks that produce usize or exit",
            )?;
            (ComposedOutcomeDestination::Usize(destination), mode)
        }
        ScalarBindingKind::Borrow {
            is_readwrite,
            inner,
        } => {
            let destination = context.next_usize_local_location()?;
            let borrow_type = Type::Borrow {
                is_readwrite: *is_readwrite,
                inner: Box::new(inner.clone()),
            };
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_borrow_expression_to_location(
                        expression,
                        destination,
                        &borrow_type,
                        context,
                    )
                },
                "IR can only lower borrow composed-outcome fallbacks that produce a matching borrow or exit",
            )?;
            (ComposedOutcomeDestination::Borrow(destination), mode)
        }
        ScalarBindingKind::Bool => {
            let destination = context.next_bool_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_bool_expression_to_location(expression, destination, context, "E8008")
                },
                "IR can only lower bool composed-outcome fallbacks that produce bool or exit",
            )?;
            (ComposedOutcomeDestination::Bool(destination), mode)
        }
        ScalarBindingKind::Str => {
            let destination = context.next_str_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_str_expression_to_location(expression, destination, context)
                },
                "IR can only lower &str composed-outcome fallbacks that produce &str or exit",
            )?;
            (ComposedOutcomeDestination::Str(destination), mode)
        }
        ScalarBindingKind::Slice(_) => {
            let destination = context.next_slice_local_location()?;
            let mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_slice_expression_to_location(expression, destination, context)
                },
                "IR can only lower slice composed-outcome fallbacks that produce a slice or exit",
            )?;
            (ComposedOutcomeDestination::Slice(destination), mode)
        }
    };

    let outer_mode = match outer_handler {
        ComposedOuterHandler::Mode(mode) => mode,
        ComposedOuterHandler::Catch(catch) => lower_composed_value_catch_failure_mode(
            catch,
            &kind,
            destination,
            &inner_mode,
            &expression_context,
        )?,
    };
    let instructions = lower_composed_outcome_call(
        call,
        destination,
        &expression_context,
        &mut temporaries,
        outer_mode,
        inner_mode,
    )?;
    match kind {
        ScalarBindingKind::I32 => context.define_i32_local(statement.name.clone()),
        ScalarBindingKind::U8 => context.define_u8_local(statement.name.clone()),
        ScalarBindingKind::Usize => context.define_usize_local(statement.name.clone()),
        ScalarBindingKind::Borrow {
            is_readwrite,
            inner,
        } => context.define_borrow_local(statement.name.clone(), is_readwrite, inner),
        ScalarBindingKind::Bool => context.define_bool_local(statement.name.clone()),
        ScalarBindingKind::Str => context.define_str_local(statement.name.clone()),
        ScalarBindingKind::Slice(info) => {
            context.define_slice_local(statement.name.clone(), info.element_kind, info.element_type)
        }
    }
    Ok(instructions)
}

fn lower_composed_value_catch_failure_mode(
    catch: &CatchExpr,
    kind: &ScalarBindingKind,
    destination: ComposedOutcomeDestination,
    absence_mode: &OutcomeFailureMode,
    context: &LoweringContext,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    lower_optional_value_catch_failure_mode(
        catch,
        destination,
        absence_mode,
        context,
        kind.abi_word_count(),
        |result, context| match (kind, destination) {
            (ScalarBindingKind::I32, ComposedOutcomeDestination::I32(destination)) => {
                lower_i32_expression_to_location(result, destination, context)
            }
            (ScalarBindingKind::U8, ComposedOutcomeDestination::U8(destination)) => {
                lower_u8_expression_to_location(result, destination, context)
            }
            (ScalarBindingKind::Usize, ComposedOutcomeDestination::Usize(destination)) => {
                lower_usize_expression_to_location(result, destination, context)
            }
            (
                ScalarBindingKind::Borrow {
                    is_readwrite,
                    inner,
                },
                ComposedOutcomeDestination::Borrow(destination),
            ) => lower_borrow_expression_to_location(
                result,
                destination,
                &Type::Borrow {
                    is_readwrite: *is_readwrite,
                    inner: Box::new(inner.clone()),
                },
                context,
            ),
            (ScalarBindingKind::Bool, ComposedOutcomeDestination::Bool(destination)) => {
                lower_bool_expression_to_location(result, destination, context, "E8008")
            }
            (ScalarBindingKind::Str, ComposedOutcomeDestination::Str(destination)) => {
                lower_str_expression_to_location(result, destination, context)
            }
            (ScalarBindingKind::Slice(_), ComposedOutcomeDestination::Slice(destination)) => {
                lower_slice_expression_to_location(result, destination, context)
            }
            _ => Err(unsupported_binding_diagnostic(
                "composed `catch` destination does not match its payload",
            )),
        },
    )
}

pub(super) fn lower_optional_value_catch_failure_mode<F>(
    catch: &CatchExpr,
    destination: ComposedOutcomeDestination,
    absence_mode: &OutcomeFailureMode,
    context: &LoweringContext,
    reserved_abi_words: usize,
    mut lower_payload: F,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
{
    lower_value_catch_failure_mode_with_result(
        catch,
        context,
        reserved_abi_words,
        None,
        |result, catch_context| {
            lower_optional_catch_result(
                result,
                destination,
                absence_mode,
                catch_context,
                &mut lower_payload,
            )
        },
        "native lowering can only lower `catch` fallbacks that produce the optional success value or exit",
    )
}

fn lower_optional_catch_result<F>(
    result: &Expr,
    destination: ComposedOutcomeDestination,
    absence_mode: &OutcomeFailureMode,
    context: &LoweringContext,
    lower_payload: &mut F,
) -> Result<LoweredFallbackResult, Vec<Diagnostic>>
where
    F: FnMut(&Expr, &LoweringContext) -> Result<Vec<Instruction>, Vec<Diagnostic>>,
{
    if matches!(unwrap_group(result), Expr::NoneLiteral(_)) {
        return match absence_mode {
            OutcomeFailureMode::Handle { instructions } => {
                Ok(LoweredFallbackResult::Terminate(instructions.clone()))
            }
            OutcomeFailureMode::Recover { instructions } => {
                Ok(LoweredFallbackResult::Continue(instructions.clone()))
            }
            _ => Err(unsupported_binding_diagnostic(
                "composed `catch` absence received an invalid handler",
            )),
        };
    }

    if let Some(instructions) =
        lower_stored_outcome_expression(result, destination, context, absence_mode.clone())?
    {
        return Ok(LoweredFallbackResult::Continue(instructions));
    }

    if let Expr::Call(call) = unwrap_group(result)
        && let Some((target, _)) = context.direct_call_target_and_name(call)
        && matches!(context.call_return_type(&target), Some(Type::Optional(_)))
    {
        let mut temporaries = TemporaryAllocator::new(context)?;
        let instructions = match destination {
            ComposedOutcomeDestination::I32(destination) => lower_fallible_i32_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::U8(destination) => lower_fallible_u8_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Usize(destination) => lower_fallible_usize_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Borrow(destination) => lower_fallible_borrow_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Bool(destination) => lower_fallible_bool_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Str(destination) => lower_fallible_str_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Slice(destination) => lower_fallible_slice_normal_call(
                call,
                destination,
                context,
                &mut temporaries,
                absence_mode.clone(),
            ),
            ComposedOutcomeDestination::Integer { .. } => Err(unsupported_binding_diagnostic(
                "composed `catch` destination does not match its payload",
            )),
        }?;
        return Ok(LoweredFallbackResult::Continue(instructions));
    }

    let instructions = lower_payload(result, context)?;
    Ok(LoweredFallbackResult::Continue(instructions))
}

pub(super) fn lower_otherwise_scalar_call_binding(
    statement: &BindingStmt,
    call: &CallExpr,
    fallback: &Block,
    kind: ScalarBindingKind,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let expression_context = context.with_reserved_local_abi_words(kind.abi_word_count());
    let mut temporaries = TemporaryAllocator::new(&expression_context)?;
    match kind {
        ScalarBindingKind::I32 => {
            let destination = context.next_i32_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_i32_expression_to_location(expression, destination, context)
                },
                "native lowering can only lower i32 `otherwise` fallback blocks that produce an i32 value or exit",
            )?;
            let instructions = lower_fallible_i32_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_i32_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::U8 => {
            let destination = context.next_u8_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_u8_expression_to_location(expression, destination, context)
                },
                "native lowering can only lower u8 `otherwise` fallback blocks that produce a u8 value or exit",
            )?;
            let instructions = lower_fallible_u8_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_u8_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Usize => {
            let destination = context.next_usize_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_usize_expression_to_location(expression, destination, context)
                },
                "native lowering can only lower usize `otherwise` fallback blocks that produce a usize value or exit",
            )?;
            let instructions = lower_fallible_usize_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_usize_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Borrow {
            is_readwrite,
            inner,
        } => {
            let destination = context.next_usize_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_borrow_expression_to_location(
                        expression,
                        destination,
                        &Type::Borrow {
                            is_readwrite,
                            inner: Box::new(inner.clone()),
                        },
                        context,
                    )
                },
                "native lowering can only lower borrow `otherwise` fallbacks that produce a matching borrow or exit",
            )?;
            let instructions = lower_fallible_borrow_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_borrow_local(statement.name.clone(), is_readwrite, inner);
            Ok(instructions)
        }
        ScalarBindingKind::Bool => {
            let destination = context.next_bool_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_bool_expression_to_location(expression, destination, context, "E8008")
                },
                "native lowering can only lower bool `otherwise` fallback blocks that produce a bool value or exit",
            )?;
            let instructions = lower_fallible_bool_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_bool_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Str => {
            let destination = context.next_str_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_str_expression_to_location(expression, destination, context)
                },
                "native lowering can only lower &str `otherwise` fallback blocks that produce a &str value or exit",
            )?;
            let instructions = lower_fallible_str_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_str_local(statement.name.clone());
            Ok(instructions)
        }
        ScalarBindingKind::Slice(info) => {
            let destination = context.next_slice_local_location()?;
            let failure_mode = lower_otherwise_recover_or_handle_failure_mode(
                fallback,
                &expression_context,
                loop_control,
                |expression, context| {
                    lower_slice_expression_to_location(expression, destination, context)
                },
                "native lowering can only lower slice `otherwise` fallback blocks that produce a slice value or exit",
            )?;
            let instructions = lower_fallible_slice_normal_call(
                call,
                destination,
                &expression_context,
                &mut temporaries,
                failure_mode,
            )?;
            context.define_slice_local(
                statement.name.clone(),
                info.element_kind,
                info.element_type,
            );
            Ok(instructions)
        }
    }
}

pub(super) fn lower_otherwise_aggregate_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Otherwise(otherwise) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    let Expr::Call(call) = unwrap_group(&otherwise.value) else {
        return Ok(None);
    };

    let Some((_root_source, resolved)) = context.resolved_calls() else {
        return Err(unsupported_binding_diagnostic(
            "native lowering cannot lower aggregate `otherwise` bindings without resolved call information",
        ));
    };
    let Some(return_type) = context.call_return_type_expr(call) else {
        return Ok(None);
    };
    if !return_type_expr_is_top_level_optional(&return_type, resolved) {
        return Ok(None);
    }
    let success_abi_value =
        top_level_optional_success_abi_value_with_resolver(&return_type, resolved, |source| {
            context.resolved_source(source)
        });

    let Some((target, call_name)) = context.direct_call_target_and_name(call) else {
        return Ok(None);
    };
    let Some(Type::Optional(success_type)) = context.call_return_type(&target).cloned() else {
        return Ok(None);
    };
    let Some(layout) = aggregate_type_layout(success_type.as_ref()) else {
        return Ok(None);
    };

    let is_copy = call_success_type_is_copy_aggregate_value(call, context);
    let drop_kind = call_success_aggregate_drop(call, context);
    let fields = call_success_aggregate_fields(call, context);
    let slot_index =
        context.define_aggregate_local(statement.name.clone(), layout, is_copy, drop_kind, fields);
    // The destination becomes initialized only on call success or after a
    // value-producing fallback. An exiting fallback must not drop stale bytes
    // left in this reusable slot by an earlier loop iteration.
    let mut failure_context = context.clone();
    failure_context.mark_aggregate_local_moved(&statement.name);
    let failure_mode = lower_otherwise_aggregate_failure_mode(
        &otherwise.fallback,
        layout,
        success_abi_value.as_ref().map(|value| &value.ty),
        AggregateLocation::Slot(slot_index),
        resolved,
        &failure_context,
        loop_control,
        &unsupported_assignment_diagnostic,
    )?;
    let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
    let (mut argument_instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &call_name, context)?;
    instructions.append(&mut argument_instructions);
    push_fallible_aggregate_call_instruction(
        &mut instructions,
        success_type.as_ref(),
        AggregateLocation::Slot(slot_index),
        target,
        arguments,
        layout,
        failure_mode,
    );
    Ok(Some(instructions))
}

pub(super) fn lower_otherwise_aggregate_failure_mode(
    fallback: &Block,
    layout: ValueLayout,
    expected_abi_type: Option<&AbiType>,
    destination: AggregateLocation,
    resolved: &ResolveOutput,
    context: &LoweringContext,
    loop_control: Option<LoopControlContext<'_>>,
    unsupported_diagnostic: &impl Fn() -> Vec<Diagnostic>,
) -> Result<OutcomeFailureMode, Vec<Diagnostic>> {
    lower_otherwise_recover_or_handle_failure_mode(
        fallback,
        context,
        loop_control,
        |expression, context| {
            if let Expr::ArrayLiteral(literal) = unwrap_group(expression) {
                let Some(expected_abi_type) = expected_abi_type else {
                    return Err(unsupported_diagnostic());
                };
                return lower_aggregate_array_literal_to_location(
                    literal,
                    expected_abi_type,
                    layout,
                    destination,
                    "E8008",
                    "`otherwise` binding fallbacks",
                    resolved,
                    context,
                )
                .map_err(|_| unsupported_diagnostic());
            }
            lower_aggregate_member_value_assignment(destination, 0, layout, expression, context)
                .map_err(|_| unsupported_diagnostic())
        },
        "native lowering can only lower aggregate `otherwise` fallback blocks with supported aggregate values or exits",
    )
}
