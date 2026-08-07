use super::*;

pub(super) fn lower_borrow_argument(
    argument: &Expr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowArgument), Vec<Diagnostic>> {
    if context.coercion_plan(argument.span()).is_some() {
        let destination = temporaries.next_usize()?;
        let instructions = super::super::lower_borrow_coercion_to_location_with_temporaries(
            argument,
            destination,
            context,
            temporaries,
        )
        .expect("checked coercion plan must lower")?;
        return Ok((
            instructions,
            BorrowArgument {
                source: BorrowSource::BorrowLocal(destination),
            },
        ));
    }
    let Type::Borrow {
        is_readwrite,
        inner,
    } = parameter_type
    else {
        unreachable!("borrow argument lowering requires a borrow parameter type");
    };

    let argument = match unwrap_group(argument) {
        Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
            unwrap_group(&unary.operand)
        }
        argument => argument,
    };
    let (instructions, source) = match argument {
        Expr::Borrow(borrow) => {
            if borrow.is_readwrite != *is_readwrite {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            }
            lower_borrow_source_from_expression(
                &borrow.expression,
                inner,
                *is_readwrite,
                parameter_type,
                callee_name,
                context,
                temporaries,
            )?
        }
        Expr::Identifier(identifier)
            if context.borrow_parameter(&identifier.name).is_some()
                || context.borrow_local(&identifier.name).is_some()
                || context
                    .aggregate_borrow_parameter(&identifier.name)
                    .is_some() =>
        {
            (
                Vec::new(),
                lower_borrow_source_from_identifier(
                    &identifier.name,
                    inner,
                    parameter_type,
                    callee_name,
                    context,
                )?,
            )
        }
        _ => {
            return Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            ));
        }
    };

    Ok((instructions, BorrowArgument { source }))
}

pub(super) fn lower_implicit_receiver_borrow_argument(
    argument: &Expr,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowArgument), Vec<Diagnostic>> {
    let Type::Borrow { inner, .. } = parameter_type else {
        unreachable!("receiver borrow argument lowering requires a borrow parameter type");
    };

    let is_readwrite = matches!(
        parameter_type,
        Type::Borrow {
            is_readwrite: true,
            ..
        }
    );
    let (instructions, source) = lower_borrow_source_from_expression(
        argument,
        inner,
        is_readwrite,
        parameter_type,
        callee_name,
        context,
        temporaries,
    )?;

    Ok((instructions, BorrowArgument { source }))
}

pub(in crate::ir::lower) fn lower_borrow_source_from_expression(
    expression: &Expr,
    inner: &Type,
    is_readwrite: bool,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    if context.coercion_plan(expression.span()).is_some() {
        let destination = temporaries.next_usize()?;
        let instructions = super::super::lower_borrow_coercion_to_location_with_temporaries(
            expression,
            destination,
            context,
            temporaries,
        )
        .expect("checked coercion plan must lower")?;
        return Ok((instructions, BorrowSource::BorrowLocal(destination)));
    }
    lower_borrow_source_from_expression_without_coercion(
        expression,
        inner,
        is_readwrite,
        parameter_type,
        callee_name,
        context,
        temporaries,
    )
}

pub(in crate::ir::lower) fn lower_borrow_source_from_expression_without_coercion(
    expression: &Expr,
    inner: &Type,
    is_readwrite: bool,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => Ok((
            Vec::new(),
            lower_borrow_source_from_identifier(
                &identifier.name,
                inner,
                parameter_type,
                callee_name,
                context,
            )?,
        )),
        Expr::Member(_) => lower_borrow_source_from_aggregate_member_expression(
            expression,
            inner,
            is_readwrite,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Index(index) => lower_borrow_source_from_slice_index_expression(
            index,
            inner,
            is_readwrite,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        Expr::Call(call) if call_returns_borrow(call, context) => {
            let destination = temporaries.next_usize()?;
            let instructions = lower_borrow_normal_call(call, destination, context, temporaries)?;
            Ok((instructions, BorrowSource::BorrowLocal(destination)))
        }
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_outcome_borrow_call_source(
                call,
                propagating_outcome_mode(&propagation.expression, context)?,
                context,
                temporaries,
            )
        }
        Expr::Force(force) => {
            let Expr::Call(call) = unwrap_group(&force.expression) else {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            lower_outcome_borrow_call_source(call, OutcomeFailureMode::Trap, context, temporaries)
        }
        Expr::Catch(catch) => {
            let Expr::Call(call) = unwrap_group(&catch.expression) else {
                return Err(unsupported_borrow_argument_diagnostic(
                    callee_name,
                    parameter_type,
                ));
            };
            let destination = temporaries.next_usize()?;
            let failure_mode = lower_catch_failure_mode(
                catch,
                context,
                usize_destination_reserved_abi_words(destination),
            )?;
            let instructions = lower_fallible_borrow_normal_call(
                call,
                destination,
                context,
                temporaries,
                failure_mode,
            )?;
            Ok((instructions, BorrowSource::BorrowLocal(destination)))
        }
        _ if !is_readwrite => lower_readonly_temporary_borrow_source(
            expression,
            inner,
            parameter_type,
            callee_name,
            context,
            temporaries,
        ),
        _ => Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
}

fn call_returns_borrow(call: &CallExpr, context: &LoweringContext) -> bool {
    context
        .direct_call_target_and_name(call)
        .and_then(|(target, _)| context.call_return_type(&target))
        .is_some_and(|return_type| matches!(return_type, Type::Borrow { .. }))
}

fn lower_outcome_borrow_call_source(
    call: &CallExpr,
    failure_mode: OutcomeFailureMode,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    let destination = temporaries.next_usize()?;
    let instructions =
        lower_fallible_borrow_normal_call(call, destination, context, temporaries, failure_mode)?;
    Ok((instructions, BorrowSource::BorrowLocal(destination)))
}

fn lower_borrow_source_from_slice_index_expression(
    expression: &IndexExpr,
    inner: &Type,
    _is_readwrite: bool,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    let element_kind = slice_index_borrow_element_kind(&expression.object, context);
    let Some(element) = slice_element_address_kind_for_borrow(element_kind, inner) else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    let source = lower_slice_expression_to_value(&expression.object, context, temporaries)?;
    let index = lower_usize_expression_to_value(&expression.index, context, temporaries)?;
    let mut instructions = source.instructions;
    instructions.extend(index.instructions);
    let index = materialize_slice_borrow_index(&mut instructions, index.value, temporaries)?;

    let SliceValue::Location(source) = source.value else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };

    Ok((
        instructions,
        BorrowSource::SliceIndex {
            source,
            index,
            element,
        },
    ))
}

pub(super) fn materialize_slice_borrow_index(
    instructions: &mut Vec<Instruction>,
    value: UsizeValue,
    temporaries: &mut TemporaryAllocator,
) -> Result<SliceElementIndex, Vec<Diagnostic>> {
    match value {
        UsizeValue::Const(value) => Ok(SliceElementIndex::Const(value)),
        UsizeValue::Location(location) => Ok(SliceElementIndex::Location(location)),
        value => {
            let destination = temporaries.next_usize()?;
            instructions.push(Instruction::SetUsize { destination, value });
            Ok(SliceElementIndex::Location(destination))
        }
    }
}

fn slice_index_borrow_element_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    match unwrap_group(expression) {
        Expr::Identifier(identifier) => context
            .slice_element_kind(&identifier.name)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Call(call) => call_return_slice_element_kind(call, context)
            .unwrap_or(TypecheckSliceElementKind::Other),
        Expr::Member(member) => match aggregate_member_field_kind_from_member(member, context)
            .ok()
            .flatten()
        {
            Some(AggregateFieldKind::Slice(info)) => info.element_kind,
            Some(_) => TypecheckSliceElementKind::Other,
            None => TypecheckSliceElementKind::Other,
        },
        Expr::Propagate(propagation) => {
            slice_index_borrow_fallible_element_kind(unwrap_group(&propagation.expression), context)
        }
        Expr::Force(force) => {
            slice_index_borrow_fallible_element_kind(unwrap_group(&force.expression), context)
        }
        Expr::Catch(catch) => {
            slice_index_borrow_fallible_element_kind(unwrap_group(&catch.expression), context)
        }
        Expr::Group(group) => slice_index_borrow_element_kind(&group.expression, context),
        _ => TypecheckSliceElementKind::Other,
    }
}

fn slice_index_borrow_fallible_element_kind(
    expression: &Expr,
    context: &LoweringContext,
) -> TypecheckSliceElementKind {
    let Expr::Call(call) = expression else {
        return TypecheckSliceElementKind::Other;
    };
    call_success_slice_element_kind(call, context).unwrap_or(TypecheckSliceElementKind::Other)
}

fn call_return_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&return_type, resolved),
    ))
}

fn call_success_slice_element_kind(
    call: &CallExpr,
    context: &LoweringContext,
) -> Option<TypecheckSliceElementKind> {
    let (_root_source, resolved) = context.resolved_calls()?;
    let return_type = context.call_return_type_expr(call)?;
    let crate::ast::TypeExpr::Fallible(fallible) = return_type else {
        return None;
    };
    Some(slice_element_kind_from_type(
        view_element_type_from_type_expr(&fallible.success, resolved),
    ))
}

fn slice_element_kind_from_type(ty: Option<Type>) -> TypecheckSliceElementKind {
    match ty {
        Some(Type::I32) => TypecheckSliceElementKind::I32,
        Some(Type::U8) => TypecheckSliceElementKind::U8,
        Some(Type::Usize) => TypecheckSliceElementKind::Usize,
        Some(Type::Bool) => TypecheckSliceElementKind::Bool,
        Some(Type::Str) => TypecheckSliceElementKind::Str,
        _ => TypecheckSliceElementKind::Other,
    }
}

fn slice_element_address_kind_for_borrow(
    element: TypecheckSliceElementKind,
    inner: &Type,
) -> Option<SliceElementAddressKind> {
    match (element, inner) {
        (TypecheckSliceElementKind::U8, Type::U8) => Some(SliceElementAddressKind::U8),
        (TypecheckSliceElementKind::I32, Type::I32) => Some(SliceElementAddressKind::I32),
        (TypecheckSliceElementKind::Usize, Type::Usize) => Some(SliceElementAddressKind::Usize),
        (TypecheckSliceElementKind::Bool, Type::Bool) => Some(SliceElementAddressKind::Bool),
        (TypecheckSliceElementKind::Str, Type::Str) => Some(SliceElementAddressKind::Str),
        (
            TypecheckSliceElementKind::Other,
            Type::Aggregate { layout } | Type::DirectAggregate { layout, .. },
        ) => u32::try_from(layout.size)
            .ok()
            .filter(|stride| *stride != 0)
            .map(|stride| SliceElementAddressKind::Aggregate { stride }),
        _ => None,
    }
}

fn lower_borrow_source_from_identifier(
    identifier_name: &str,
    inner: &Type,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
) -> Result<BorrowSource, Vec<Diagnostic>> {
    let requires_readwrite = matches!(
        parameter_type,
        Type::Borrow {
            is_readwrite: true,
            ..
        }
    );
    if let Some(borrow) = context.borrow_parameter(identifier_name)
        && borrow.inner == *inner
        && (!requires_readwrite || borrow.is_readwrite)
    {
        return Ok(BorrowSource::BorrowParameter(borrow.parameter_index));
    }
    if let Some((location, is_readwrite, local_inner)) = context.borrow_local(identifier_name)
        && local_inner == inner
        && (!requires_readwrite || is_readwrite)
    {
        return Ok(BorrowSource::BorrowLocal(location));
    }

    match inner {
        Type::I32 => match context.i32_location(identifier_name) {
            Some(I32Location::Local(index)) => Ok(BorrowSource::I32(I32Location::Local(index))),
            Some(I32Location::Parameter(index)) => {
                Ok(BorrowSource::I32(I32Location::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::U8 => match context.u8_location(identifier_name) {
            Some(U8Location::Local(index)) => Ok(BorrowSource::U8(U8Location::Local(index))),
            Some(U8Location::Parameter(index)) => {
                Ok(BorrowSource::U8(U8Location::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Usize => match context.usize_location(identifier_name) {
            Some(UsizeLocation::Local(index)) => {
                Ok(BorrowSource::Usize(UsizeLocation::Local(index)))
            }
            Some(UsizeLocation::Parameter(index)) => {
                Ok(BorrowSource::Usize(UsizeLocation::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Bool => match context.bool_location(identifier_name) {
            Some(BoolLocation::Local(index)) => Ok(BorrowSource::Bool(BoolLocation::Local(index))),
            Some(BoolLocation::Parameter(index)) => {
                Ok(BorrowSource::Bool(BoolLocation::Parameter(index)))
            }
            _ => Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            )),
        },
        Type::Aggregate {
            layout: expected_layout,
        }
        | Type::DirectAggregate {
            layout: expected_layout,
            ..
        } => {
            if let Some((slot_index, layout)) = context.aggregate_slot(identifier_name)
                && layout == *expected_layout
            {
                return Ok(BorrowSource::AggregateSlot(slot_index));
            }

            let required_readwrite = matches!(
                parameter_type,
                Type::Borrow {
                    is_readwrite: true,
                    ..
                }
            );
            if let Some(borrow) = context.aggregate_borrow_parameter(identifier_name)
                && borrow.layout == *expected_layout
                && (!required_readwrite || borrow.is_readwrite)
            {
                return Ok(BorrowSource::AggregateParameter(borrow.parameter_index));
            }

            Err(unsupported_borrow_argument_diagnostic(
                callee_name,
                parameter_type,
            ))
        }
        _ => Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
}

fn lower_borrow_source_from_aggregate_member_expression(
    expression: &Expr,
    inner: &Type,
    is_readwrite: bool,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    let Some(field) = lower_aggregate_member_field_access(expression, context, temporaries)? else {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    };
    if is_readwrite && !field.is_readwrite {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }
    if !aggregate_field_matches_borrow_inner(&field.kind, inner) {
        return Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        ));
    }

    let source = match field.source {
        AggregateLocation::Slot(slot_index) => Ok(BorrowSource::AggregateSlotField {
            slot_index,
            offset: field.offset,
        }),
        AggregateLocation::Parameter(parameter_index) => {
            Ok(BorrowSource::AggregateParameterField {
                parameter_index,
                offset: field.offset,
            })
        }
        AggregateLocation::Return
        | AggregateLocation::DirectReturn
        | AggregateLocation::DirectParameter { .. } => Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }?;
    Ok((field.instructions, source))
}

fn lower_readonly_temporary_borrow_source(
    expression: &Expr,
    inner: &Type,
    parameter_type: &Type,
    callee_name: &str,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<(Vec<Instruction>, BorrowSource), Vec<Diagnostic>> {
    match inner {
        Type::I32 => {
            let lowered = lower_i32_expression_to_value(expression, context, temporaries)?;
            let destination = temporaries.next_i32()?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetI32 {
                destination,
                value: lowered.value,
            });
            Ok((instructions, BorrowSource::I32(destination)))
        }
        Type::U8 => {
            let lowered = lower_u8_expression_to_value(expression, context, temporaries)?;
            let destination = temporaries.next_u8()?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetU8 {
                destination,
                value: lowered.value,
            });
            Ok((instructions, BorrowSource::U8(destination)))
        }
        Type::Usize => {
            let lowered = lower_usize_expression_to_value(expression, context, temporaries)?;
            let destination = temporaries.next_usize()?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetUsize {
                destination,
                value: lowered.value,
            });
            Ok((instructions, BorrowSource::Usize(destination)))
        }
        Type::Bool => {
            let lowered = lower_bool_expression_to_value_with_temporaries(
                expression,
                context,
                "E8006",
                temporaries,
            )?;
            let destination = temporaries.next_bool()?;
            let mut instructions = lowered.instructions;
            instructions.push(Instruction::SetBool {
                destination,
                value: lowered.value,
            });
            Ok((instructions, BorrowSource::Bool(destination)))
        }
        Type::Aggregate { .. } | Type::DirectAggregate { .. } => {
            let (instructions, source) = lower_aggregate_argument_source(
                expression,
                false,
                inner,
                None,
                callee_name,
                context,
                temporaries,
            )?;
            match source {
                AggregateArgumentSource::Slot(slot_index) => {
                    Ok((instructions, BorrowSource::AggregateSlot(slot_index)))
                }
            }
        }
        _ => Err(unsupported_borrow_argument_diagnostic(
            callee_name,
            parameter_type,
        )),
    }
}

fn aggregate_field_matches_borrow_inner(kind: &AggregateFieldKind, inner: &Type) -> bool {
    match (kind, inner) {
        (AggregateFieldKind::I32, Type::I32)
        | (AggregateFieldKind::U8, Type::U8)
        | (AggregateFieldKind::Usize, Type::Usize)
        | (AggregateFieldKind::Bool, Type::Bool) => true,
        (AggregateFieldKind::Str, Type::Str)
        | (AggregateFieldKind::Slice(_), Type::Slice { .. }) => true,
        (AggregateFieldKind::Array { layout, .. }, Type::Aggregate { layout: expected })
        | (
            AggregateFieldKind::Array { layout, .. },
            Type::DirectAggregate {
                layout: expected, ..
            },
        ) => layout == expected,
        (AggregateFieldKind::Aggregate { layout, .. }, Type::Aggregate { layout: expected })
        | (
            AggregateFieldKind::Aggregate { layout, .. },
            Type::DirectAggregate {
                layout: expected, ..
            },
        ) => layout == expected,
        (AggregateFieldKind::Outcome { storage, .. }, Type::Aggregate { layout })
        | (AggregateFieldKind::Outcome { storage, .. }, Type::DirectAggregate { layout, .. }) => {
            storage.layout == *layout
        }
        _ => false,
    }
}

fn unsupported_borrow_argument_diagnostic(
    callee_name: &str,
    parameter_type: &Type,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8006",
        format!(
            "native lowering can only lower `{}` arguments from scalar local bindings for function `{callee_name}`",
            describe_type(parameter_type),
        ),
    )]
}
