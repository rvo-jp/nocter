use super::*;

pub(in crate::ir::lower) fn lower_str_expression_to_location(
    expression: &Expr,
    destination: StrLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut coercion_temporaries = TemporaryAllocator::new(context)?;
    if let Some(lowered) =
        lower_str_coercion_to_location(expression, destination, context, &mut coercion_temporaries)
    {
        return lowered;
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_arg_raw_call(call, context) || primitive_env_entry_raw_call(call, context)
            {
                let (mut instructions, value) = if primitive_arg_raw_call(call, context) {
                    lower_arg_raw_primitive_call_to_value(call, context, &mut temporaries)?
                } else {
                    lower_env_entry_raw_primitive_call_to_value(call, context, &mut temporaries)?
                };
                instructions.push(Instruction::SetStr { destination, value });
                return Ok(instructions);
            }
            if primitive_str_from_raw_parts_call(call, context) {
                return lower_str_from_raw_parts_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            lower_str_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_str_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_outcome_mode(&propagation.expression, context)?,
        ),
        Expr::Force(force) => lower_str_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            OutcomeFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_str_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                str_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_str_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_str_expression_diagnostic)
        }
        Expr::If(statement) => lower_str_if_expression_to_location(statement, destination, context),
        Expr::IfIs(statement) => {
            lower_str_if_is_expression_to_location(statement, destination, context)
        }
        Expr::Match(statement) => lower_str_match_expression_to_location(
            statement,
            destination,
            context,
            str_destination_reserved_abi_words(destination),
        ),
        Expr::Group(group) => {
            lower_str_expression_to_location(&group.expression, destination, context)
        }
        _ => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let value = lower_str_expression_to_value(expression, context, &mut temporaries)?;
            let mut instructions = value.instructions;
            instructions.push(Instruction::SetStr {
                destination,
                value: value.value,
            });
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_slice_expression_to_location(
    expression: &Expr,
    destination: SliceLocation,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut coercion_temporaries = TemporaryAllocator::new(context)?;
    if let Some(lowered) = lower_slice_coercion_to_location(
        expression,
        destination,
        context,
        &mut coercion_temporaries,
    ) {
        return lowered;
    }
    match expression {
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if primitive_slice_from_raw_parts_call(call, context) {
                return lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            if primitive_bytes_from_str_call(call, context) {
                return lower_str_bytes_primitive_call_to_location(
                    call,
                    destination,
                    context,
                    &mut temporaries,
                );
            }
            lower_slice_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_slice_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            propagating_outcome_mode(&propagation.expression, context)?,
        ),
        Expr::Force(force) => lower_slice_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            OutcomeFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_slice_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            lower_catch_failure_mode(
                catch,
                context,
                slice_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_slice_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(unsupported_slice_expression_diagnostic)
        }
        Expr::If(statement) => {
            lower_slice_if_expression_to_location(statement, destination, context)
        }
        Expr::IfIs(statement) => {
            lower_slice_if_is_expression_to_location(statement, destination, context)
        }
        Expr::Match(statement) => lower_slice_match_expression_to_location(
            statement,
            destination,
            context,
            slice_destination_reserved_abi_words(destination),
        ),
        Expr::Group(group) => {
            lower_slice_expression_to_location(&group.expression, destination, context)
        }
        _ => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let value = lower_slice_expression_to_value(expression, context, &mut temporaries)?;
            let mut instructions = value.instructions;
            instructions.push(Instruction::SetSlice {
                destination,
                value: value.value,
            });
            Ok(instructions)
        }
    }
}

pub(in crate::ir::lower) fn lower_str_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredStrValue, Vec<Diagnostic>> {
    if context.coercion_plan(expression.span()).is_some() {
        let temporary = temporaries.next_str()?;
        return Ok(LoweredStrValue {
            instructions: lower_str_coercion_to_location(
                expression,
                temporary,
                context,
                temporaries,
            )
            .expect("checked coercion plan must lower")?,
            value: StrValue::Location(temporary),
        });
    }
    match expression {
        Expr::Call(call) => {
            if primitive_arg_raw_call(call, context) || primitive_env_entry_raw_call(call, context)
            {
                let (instructions, value) = if primitive_arg_raw_call(call, context) {
                    lower_arg_raw_primitive_call_to_value(call, context, temporaries)?
                } else {
                    lower_env_entry_raw_primitive_call_to_value(call, context, temporaries)?
                };
                return Ok(LoweredStrValue {
                    instructions,
                    value,
                });
            }
            let temporary = temporaries.next_str()?;
            if primitive_str_from_raw_parts_call(call, context) {
                return Ok(LoweredStrValue {
                    instructions: lower_str_from_raw_parts_primitive_call_to_location(
                        call,
                        temporary,
                        context,
                        temporaries,
                    )?,
                    value: StrValue::Location(temporary),
                });
            }
            Ok(LoweredStrValue {
                instructions: lower_str_normal_call(call, temporary, context, temporaries)?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_outcome_mode(&propagation.expression, context)?,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    OutcomeFailureMode::Trap,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_str()?;
            Ok(LoweredStrValue {
                instructions: lower_str_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        str_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_str()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredStrValue {
                instructions: lower_str_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_str_expression_diagnostic)?,
                value: StrValue::Location(temporary),
            })
        }
        Expr::Match(statement) => {
            lower_str_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_str_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_str_if_is_expression_to_value(statement, context, temporaries)
        }
        Expr::Member(_) => lower_aggregate_str_field_to_value(expression, context, temporaries),
        Expr::Index(index) => lower_str_index_expression_to_value(index, context, temporaries),
        Expr::Group(group) => {
            lower_str_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredStrValue {
            instructions: Vec::new(),
            value: lower_str_value(expression, context)?,
        }),
    }
}

pub(in crate::ir::lower) fn lower_slice_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredSliceValue, Vec<Diagnostic>> {
    if context.coercion_plan(expression.span()).is_some() {
        let temporary = temporaries.next_slice()?;
        return Ok(LoweredSliceValue {
            instructions: lower_slice_coercion_to_location(
                expression,
                temporary,
                context,
                temporaries,
            )
            .expect("checked coercion plan must lower")?,
            value: SliceValue::Location(temporary),
        });
    }
    match expression {
        Expr::Call(call) => {
            if primitive_slice_from_raw_parts_call(call, context) {
                let temporary = temporaries.next_slice()?;
                let instructions = lower_slice_from_raw_parts_primitive_call_to_location(
                    call,
                    temporary,
                    context,
                    temporaries,
                )?;
                return Ok(LoweredSliceValue {
                    instructions,
                    value: SliceValue::Location(temporary),
                });
            }
            if primitive_bytes_from_str_call(call, context) {
                let (instructions, value) =
                    lower_str_bytes_primitive_call_to_value(call, context, temporaries)?;
                return Ok(LoweredSliceValue {
                    instructions,
                    value,
                });
            }
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_normal_call(call, temporary, context, temporaries)?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    propagating_outcome_mode(&propagation.expression, context)?,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    OutcomeFailureMode::Trap,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_slice()?;
            Ok(LoweredSliceValue {
                instructions: lower_slice_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        slice_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_slice()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredSliceValue {
                instructions: lower_slice_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(unsupported_slice_expression_diagnostic)?,
                value: SliceValue::Location(temporary),
            })
        }
        Expr::Match(statement) => {
            lower_slice_match_expression_to_value(statement, context, temporaries)
        }
        Expr::If(statement) => lower_slice_if_expression_to_value(statement, context, temporaries),
        Expr::IfIs(statement) => {
            lower_slice_if_is_expression_to_value(statement, context, temporaries)
        }
        Expr::Member(_) => lower_aggregate_slice_field_to_value(expression, context, temporaries),
        Expr::Group(group) => {
            lower_slice_expression_to_value(&group.expression, context, temporaries)
        }
        _ => Ok(LoweredSliceValue {
            instructions: Vec::new(),
            value: lower_slice_value(expression, context)?,
        }),
    }
}
