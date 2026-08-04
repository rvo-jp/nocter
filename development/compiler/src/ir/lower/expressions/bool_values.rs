use super::*;

pub(in crate::ir::lower) fn lower_bool_expression_to_location(
    expression: &Expr,
    destination: BoolLocation,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if let Expr::Identifier(identifier) = expression
        && (context.borrow_parameter(&identifier.name).is_some()
            || context.borrow_local(&identifier.name).is_some()
            || context.closure_capture_field(&identifier.name).is_some())
    {
        if let Some(instructions) =
            lower_bool_borrow_binding_to_location(identifier, destination, context)
        {
            return Ok(instructions);
        }
        let mut temporaries = TemporaryAllocator::new(context)?;
        if let Some(instructions) = lower_bool_closure_capture_to_location(
            identifier,
            destination,
            context,
            &mut temporaries,
        )? {
            return Ok(instructions);
        }
    }
    match expression {
        Expr::Binary(binary) if short_circuit_bool_expression_needs_branch(binary, context) => {
            lower_short_circuit_bool_expression_to_location(
                binary,
                destination,
                context,
                diagnostic_code,
            )
        }
        Expr::Binary(binary) if str_comparison_is_lowerable(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_str_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if bool_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if u8_comparison_is_lowerable(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_u8_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Binary(binary) if usize_comparison_needs_temporaries(binary, context) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let comparison = lower_usize_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = comparison.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: comparison.value,
            });
            Ok(instructions)
        }
        Expr::Call(call) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            if let Some(value) =
                lower_builtin_is_empty_call_to_value(call, context, &mut temporaries)
            {
                let lowered = value?;
                let mut instructions = lowered.instructions;
                instructions.push(Instruction::SetBool {
                    destination,
                    value: lowered.value,
                });
                return Ok(instructions);
            }
            lower_bool_normal_call(call, destination, context, &mut temporaries)
        }
        Expr::Propagate(propagation) => lower_bool_fallible_expression_to_location(
            &propagation.expression,
            destination,
            context,
            diagnostic_code,
            propagating_outcome_mode(&propagation.expression, context)?,
        ),
        Expr::Force(force) => lower_bool_fallible_expression_to_location(
            &force.expression,
            destination,
            context,
            diagnostic_code,
            FallibleFailureMode::Trap,
        ),
        Expr::Catch(catch) => lower_bool_fallible_expression_to_location(
            &catch.expression,
            destination,
            context,
            diagnostic_code,
            lower_catch_failure_mode(
                catch,
                context,
                bool_destination_reserved_abi_words(destination),
            )?,
        ),
        Expr::Otherwise(_) => {
            lower_bool_optional_otherwise_to_location(expression, destination, context)?
                .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))
        }
        Expr::If(statement) => {
            lower_bool_if_expression_to_location(statement, destination, context, diagnostic_code)
        }
        Expr::IfIs(statement) => lower_bool_if_is_expression_to_location(
            statement,
            destination,
            context,
            diagnostic_code,
        ),
        Expr::Match(statement) => lower_bool_match_expression_to_location(
            statement,
            destination,
            context,
            diagnostic_code,
            bool_destination_reserved_abi_words(destination),
        ),
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = operand.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: BoolValue::Not(Box::new(operand.value)),
            });
            Ok(instructions)
        }
        Expr::Index(index) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            let lowered = lower_bool_index_expression_to_value(
                index,
                context,
                diagnostic_code,
                &mut temporaries,
            )?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: lowered.value,
            });
            Ok(instructions)
        }
        Expr::Member(_) => {
            let mut temporaries = TemporaryAllocator::new(context)?;
            lower_aggregate_bool_field_to_location(
                expression,
                destination,
                context,
                diagnostic_code,
                &mut temporaries,
            )
        }
        Expr::Group(group) => lower_bool_expression_to_location(
            &group.expression,
            destination,
            context,
            diagnostic_code,
        ),
        _ => Ok(vec![Instruction::SetBool {
            destination,
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }]),
    }
}

pub(in crate::ir::lower) struct LoweredBoolValue {
    pub(in crate::ir::lower) instructions: Vec<Instruction>,
    pub(in crate::ir::lower) value: BoolValue,
}

pub(in crate::ir::lower) fn lower_bool_expression_to_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    let mut temporaries = TemporaryAllocator::new(context)?;
    lower_bool_expression_to_value_with_temporaries(
        expression,
        context,
        diagnostic_code,
        &mut temporaries,
    )
}

pub(in crate::ir::lower) fn lower_bool_expression_to_value_with_temporaries(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
    temporaries: &mut TemporaryAllocator,
) -> Result<LoweredBoolValue, Vec<Diagnostic>> {
    if let Expr::Identifier(identifier) = expression {
        let destination = temporaries.next_bool()?;
        if let Some(instructions) =
            lower_bool_borrow_binding_to_location(identifier, destination, context)
        {
            return Ok(LoweredBoolValue {
                instructions,
                value: BoolValue::Location(destination),
            });
        }
        if let Some(instructions) =
            lower_bool_closure_capture_to_location(identifier, destination, context, temporaries)?
        {
            return Ok(LoweredBoolValue {
                instructions,
                value: BoolValue::Location(destination),
            });
        }
    }
    match expression {
        Expr::Binary(binary) if short_circuit_bool_expression_needs_branch(binary, context) => {
            lower_short_circuit_bool_expression_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if str_comparison_is_lowerable(binary, context) => {
            lower_str_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if bool_comparison_contains_call(binary, context) => {
            lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if bool_comparison_needs_temporaries(binary, context) => {
            lower_bool_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if u8_comparison_is_lowerable(binary, context) => {
            lower_u8_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if i32_comparison_needs_temporaries(binary, context) => {
            lower_i32_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Binary(binary) if usize_comparison_needs_temporaries(binary, context) => {
            lower_usize_comparison_to_value_with_temporaries(
                binary,
                context,
                diagnostic_code,
                temporaries,
            )
        }
        Expr::Call(call) => {
            if let Some(value) = lower_builtin_is_empty_call_to_value(call, context, temporaries) {
                return value;
            }

            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_normal_call(call, temporary, context, temporaries)?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Propagate(propagation) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &propagation.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    propagating_outcome_mode(&propagation.expression, context)?,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Force(force) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &force.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    FallibleFailureMode::Trap,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Catch(catch) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_bool_fallible_expression_to_location(
                    &catch.expression,
                    temporary,
                    context,
                    diagnostic_code,
                    lower_catch_failure_mode(
                        catch,
                        context,
                        bool_destination_reserved_abi_words(temporary),
                    )?,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Otherwise(_) => {
            let temporary = temporaries.next_bool()?;
            let expression_context = context
                .with_reserved_local_abi_words(temporaries.reserved_local_abi_words(context)?);
            Ok(LoweredBoolValue {
                instructions: lower_bool_optional_otherwise_to_location(
                    expression,
                    temporary,
                    &expression_context,
                )?
                .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code))?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::If(statement) => {
            lower_bool_if_expression_to_value(statement, context, diagnostic_code, temporaries)
        }
        Expr::IfIs(statement) => {
            lower_bool_if_is_expression_to_value(statement, context, diagnostic_code, temporaries)
        }
        Expr::Match(statement) => {
            lower_bool_match_expression_to_value(statement, context, diagnostic_code, temporaries)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
            let operand = lower_bool_expression_to_value_with_temporaries(
                &unary.operand,
                context,
                diagnostic_code,
                temporaries,
            )?;
            Ok(LoweredBoolValue {
                instructions: operand.instructions,
                value: BoolValue::Not(Box::new(operand.value)),
            })
        }
        Expr::Index(index) => {
            lower_bool_index_expression_to_value(index, context, diagnostic_code, temporaries)
        }
        Expr::Member(_) => {
            let temporary = temporaries.next_bool()?;
            Ok(LoweredBoolValue {
                instructions: lower_aggregate_bool_field_to_location(
                    expression,
                    temporary,
                    context,
                    diagnostic_code,
                    temporaries,
                )?,
                value: BoolValue::Location(temporary),
            })
        }
        Expr::Group(group) => lower_bool_expression_to_value_with_temporaries(
            &group.expression,
            context,
            diagnostic_code,
            temporaries,
        ),
        _ => Ok(LoweredBoolValue {
            instructions: Vec::new(),
            value: lower_bool_value(expression, context, diagnostic_code)?,
        }),
    }
}

pub(in crate::ir::lower) fn lower_bool_value(
    expression: &Expr,
    context: &LoweringContext,
    diagnostic_code: &'static str,
) -> Result<BoolValue, Vec<Diagnostic>> {
    match expression {
        Expr::Call(_) => Err(unsupported_non_tail_call_diagnostic()),
        Expr::BoolLiteral(literal) => match literal.value.as_str() {
            "true" => Ok(BoolValue::Const(true)),
            "false" => Ok(BoolValue::Const(false)),
            _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
        },
        Expr::Identifier(identifier) => context
            .bool_location(&identifier.name)
            .map(BoolValue::Location)
            .ok_or_else(|| unsupported_bool_expression_diagnostic(diagnostic_code)),
        Expr::Unary(unary) => lower_bool_unary_value(unary, context, diagnostic_code),
        Expr::Binary(binary) => lower_bool_binary_value(binary, context, diagnostic_code),
        Expr::Group(group) => lower_bool_value(&group.expression, context, diagnostic_code),
        _ => Err(unsupported_bool_expression_diagnostic(diagnostic_code)),
    }
}
