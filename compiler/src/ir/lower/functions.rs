use super::aggregates::{
    aggregate_fields_from_type_expr, lower_aggregate_struct_literal_to_location,
    lower_aggregate_struct_literal_to_location_with_temporaries, supported_aggregate_copy_layout,
};
use super::bindings::{lower_assignment, lower_local_binding};
use super::context::{
    AggregateBorrowParameter, AggregateFieldKind, AggregateParameterSource, FunctionNames,
    FunctionSignatures, LoweringAggregateParameter, LoweringContext, LoweringParameterSlots,
};
use super::control_flow::{lower_terminal_bool_if_statement, lower_terminal_i32_if_statement};
use super::errors::{ErrorPayload, lower_error_payload};
use super::expressions::{
    TemporaryAllocator, lower_aggregate_member_field_access, lower_bool_return_expression,
    lower_call_arguments_to_scalar_arguments, lower_i32_return_expression,
    lower_never_return_expression, lower_slice_return_expression, lower_str_return_expression,
    lower_u8_return_expression, lower_usize_return_expression, lower_void_expression_statement,
    mark_fallible_success_returns, success_return_instruction,
};
use crate::abi::{
    ARGUMENT_REGISTER_COUNT, AbiType, AbiValue, ValueClassification, abi_value_from_type_expr,
};
use crate::ast::{Expr, FunctionDecl, Parameter, Stmt, StructLiteralExpr, TypeExpr, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, CallTarget, FallibleFailureMode, Function, Instruction, Type};
use crate::resolve::ResolveOutput;
use crate::source::SourceId;

pub(super) fn lower_function(
    function: &FunctionDecl,
    target: CallTarget,
    function_signatures: FunctionSignatures,
    function_names: FunctionNames,
    root_source: SourceId,
    resolved: &ResolveOutput,
) -> Result<Function, Vec<Diagnostic>> {
    if !function.generics.parameters.is_empty() {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower non-generic functions, got `{}`",
                function.name
            ),
        )]);
    }

    let parameters = lower_scalar_parameters(function, resolved)?;
    let parameter_setup = lower_aggregate_parameter_setup(&parameters);
    let return_type = lower_function_return_type(&function.return_type, &function.name, resolved)?;
    let success_type = return_type.success_type().clone();
    let mut context = LoweringContext::new(
        function.name.clone(),
        success_type,
        function_signatures,
        parameters,
    )
    .with_function_return_type(return_type.clone())
    .with_call_resolution(root_source, resolved, function_names);
    let mut instructions = parameter_setup;
    instructions.extend(lower_function_body(
        function,
        &return_type,
        root_source,
        resolved,
        &mut context,
    )?);

    Ok(Function {
        name: function.name.clone(),
        target,
        return_type,
        instructions,
    })
}

fn lower_scalar_parameters(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> Result<LoweringParameterSlots, Vec<Diagnostic>> {
    let mut i32_parameters = Vec::new();
    let mut u8_parameters = Vec::new();
    let mut usize_parameters = Vec::new();
    let mut bool_parameters = Vec::new();
    let mut str_parameters = Vec::new();
    let mut slice_parameters = Vec::new();
    let mut aggregate_parameters = Vec::new();
    let mut aggregate_borrow_parameters = Vec::new();
    for parameter in &function.parameters.parameters {
        match lower_scalar_parameter_kind(parameter, &function.name, resolved)? {
            ScalarParameterKind::I32 => {
                i32_parameters.push(Some(parameter.name.clone()));
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::U8 => {
                i32_parameters.push(None);
                u8_parameters.push(Some(parameter.name.clone()));
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::Usize => {
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(Some(parameter.name.clone()));
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::Bool => {
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(Some(parameter.name.clone()));
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::Str => {
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(Some(parameter.name.clone()));
                slice_parameters.push(None);
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::Slice => {
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(Some(parameter.name.clone()));
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::Borrow => {
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
            }
            ScalarParameterKind::BorrowAggregate {
                layout,
                is_readwrite,
                fields,
            } => {
                let parameter_index = i32_parameters.len();
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
                aggregate_borrow_parameters.push(AggregateBorrowParameter {
                    name: parameter.name.clone(),
                    layout,
                    parameter_index,
                    is_readwrite,
                    fields,
                });
            }
            ScalarParameterKind::AggregateIndirect { layout, fields } => {
                let parameter_index = i32_parameters.len();
                i32_parameters.push(None);
                u8_parameters.push(None);
                usize_parameters.push(None);
                bool_parameters.push(None);
                str_parameters.push(None);
                slice_parameters.push(None);
                aggregate_parameters.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index: aggregate_parameters.len(),
                    source: AggregateParameterSource::Indirect { parameter_index },
                    fields,
                });
            }
            ScalarParameterKind::AggregateDirect {
                layout,
                words,
                fields,
            } => {
                let start_index = i32_parameters.len();
                for _ in 0..words {
                    i32_parameters.push(None);
                    u8_parameters.push(None);
                    usize_parameters.push(None);
                    bool_parameters.push(None);
                    str_parameters.push(None);
                    slice_parameters.push(None);
                }
                aggregate_parameters.push(LoweringAggregateParameter {
                    name: parameter.name.clone(),
                    layout,
                    slot_index: aggregate_parameters.len(),
                    source: AggregateParameterSource::Direct { start_index, words },
                    fields,
                });
            }
        }
    }

    let abi_word_count = lowered_parameter_abi_word_count(function, resolved)?;
    if abi_word_count > ARGUMENT_REGISTER_COUNT {
        return Err(vec![Diagnostic::error(
            "E8007",
            format!(
                "IR v0 can only lower up to {ARGUMENT_REGISTER_COUNT} ABI parameter words for function `{}`",
                function.name
            ),
        )]);
    }

    Ok(LoweringParameterSlots {
        i32: i32_parameters,
        u8: u8_parameters,
        usize: usize_parameters,
        bool: bool_parameters,
        str: str_parameters,
        slice: slice_parameters,
        aggregates: aggregate_parameters,
        aggregate_borrows: aggregate_borrow_parameters,
    })
}

fn lower_aggregate_parameter_setup(parameters: &LoweringParameterSlots) -> Vec<Instruction> {
    let mut instructions = Vec::new();
    for parameter in &parameters.aggregates {
        instructions.push(Instruction::ReserveAggregateSlot {
            slot_index: parameter.slot_index,
            layout: parameter.layout,
        });
        let source = match parameter.source {
            AggregateParameterSource::Indirect { parameter_index } => {
                AggregateLocation::Parameter(parameter_index)
            }
            AggregateParameterSource::Direct { start_index, .. } => {
                AggregateLocation::DirectParameter { start_index }
            }
        };
        instructions.push(Instruction::CopyAggregate {
            destination: AggregateLocation::Slot(parameter.slot_index),
            source,
            layout: parameter.layout,
        });
    }
    instructions
}

fn lowered_parameter_abi_word_count(
    function: &FunctionDecl,
    resolved: &ResolveOutput,
) -> Result<usize, Vec<Diagnostic>> {
    let mut count = 0_usize;
    for parameter in &function.parameters.parameters {
        let value = abi_value_from_type_expr(&parameter.ty, resolved).map_err(|_error| {
            vec![Diagnostic::error(
                "E8007",
                format!(
                    "IR v0 cannot classify parameter `{}` of function `{}` as a supported ABI value",
                    parameter.name, function.name
                ),
            )]
        })?;
        count = count
            .checked_add(value.parameter_abi_word_count())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 parameter ABI word count overflows for function `{}`",
                        function.name
                    ),
                )]
            })?;
    }
    Ok(count)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScalarParameterKind {
    I32,
    U8,
    Usize,
    Bool,
    Str,
    Slice,
    Borrow,
    BorrowAggregate {
        layout: crate::abi::ValueLayout,
        is_readwrite: bool,
        fields: Vec<super::context::AggregateField>,
    },
    AggregateIndirect {
        layout: crate::abi::ValueLayout,
        fields: Vec<super::context::AggregateField>,
    },
    AggregateDirect {
        layout: crate::abi::ValueLayout,
        words: usize,
        fields: Vec<super::context::AggregateField>,
    },
}

fn lower_scalar_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    match &parameter.ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(ScalarParameterKind::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Ok(ScalarParameterKind::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => {
            Ok(ScalarParameterKind::Usize)
        }
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(ScalarParameterKind::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Ok(ScalarParameterKind::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => {
            Ok(ScalarParameterKind::Slice)
        }
        TypeExpr::Borrow(borrow)
            if matches!(
                borrow_inner_type(&borrow.inner, resolved),
                Some(Type::Aggregate { .. } | Type::DirectAggregate { .. })
            ) =>
        {
            lower_aggregate_borrow_parameter_kind(parameter, function_name, resolved)
        }
        TypeExpr::Borrow(borrow) if borrow_inner_type(&borrow.inner, resolved).is_some() => {
            Ok(ScalarParameterKind::Borrow)
        }
        _ => lower_aggregate_parameter_kind(parameter, function_name, resolved),
    }
}

fn lower_aggregate_borrow_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let TypeExpr::Borrow(borrow) = &parameter.ty else {
        unreachable!("aggregate borrow parameter lowering requires a borrow type");
    };
    let value = abi_value_from_type_expr(&borrow.inner, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&borrow.inner, resolved)
        .ok_or_else(|| unsupported_parameter_type_diagnostic(function_name))?;
    Ok(ScalarParameterKind::BorrowAggregate {
        layout: value.layout,
        is_readwrite: borrow.is_readwrite,
        fields,
    })
}

fn lower_aggregate_parameter_kind(
    parameter: &Parameter,
    function_name: &str,
    resolved: &ResolveOutput,
) -> Result<ScalarParameterKind, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(&parameter.ty, resolved)
        .map_err(|_error| unsupported_parameter_type_diagnostic(function_name))?;
    if !matches!(value.ty, AbiType::Struct(_)) || !supported_aggregate_copy_layout(value.layout) {
        return Err(unsupported_parameter_type_diagnostic(function_name));
    }
    let fields = aggregate_fields_from_type_expr(&parameter.ty, resolved)
        .ok_or_else(|| unsupported_parameter_type_diagnostic(function_name))?;
    match value.classification {
        ValueClassification::Indirect => Ok(ScalarParameterKind::AggregateIndirect {
            layout: value.layout,
            fields,
        }),
        ValueClassification::Direct { words } => Ok(ScalarParameterKind::AggregateDirect {
            layout: value.layout,
            words,
            fields,
        }),
    }
}

fn unsupported_parameter_type_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower `i32`, `u8`, `usize`, `bool`, `&str`, `&[u8]`, `&+[u8]`, scalar borrow parameters, aggregate borrow parameters, and aggregate value parameters whose final ABI word is 1, 2, 4, or 8 bytes for function `{function_name}`"
        ),
    )]
}

fn lower_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Ok(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Ok(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Ok(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Ok(Type::Bool),
        TypeExpr::Borrow(borrow)
            if !borrow.is_readwrite
                && matches!(borrow.inner.as_ref(), TypeExpr::Reference(reference) if reference.name == "str") =>
        {
            Ok(Type::Str)
        }
        TypeExpr::Borrow(borrow) if is_u8_slice_data_type(&borrow.inner) => Ok(Type::Slice {
            is_readwrite: borrow.is_readwrite,
        }),
        TypeExpr::Reference(reference) if reference.name == "void" => Ok(Type::Void),
        TypeExpr::Reference(reference) if reference.name == "never" => Ok(Type::Never),
        TypeExpr::Fallible(fallible) => {
            lower_function_return_type(&fallible.success, name, resolved)
                .map(|success| Type::Fallible(Box::new(success)))
        }
        _ => lower_aggregate_function_return_type(ty, name, resolved),
    }
}

fn lower_aggregate_function_return_type(
    ty: &TypeExpr,
    name: &str,
    resolved: &ResolveOutput,
) -> Result<Type, Vec<Diagnostic>> {
    let value = abi_value_from_type_expr(ty, resolved)
        .map_err(|_error| unsupported_function_return_type_diagnostic(name))?;
    aggregate_type_from_abi_value(&value)
        .ok_or_else(|| unsupported_function_return_type_diagnostic(name))
}

fn unsupported_function_return_type_diagnostic(name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{name}` return type `i32`, `u8`, `usize`, `bool`, `&str`, `&[u8]`, `&+[u8]`, `void`, `never`, aggregates, or a fallible form of those types"
        ),
    )]
}

fn is_u8_slice_data_type(ty: &TypeExpr) -> bool {
    matches!(
        ty,
        TypeExpr::View(view)
            if !view.is_readwrite
                && matches!(view.element.as_ref(), TypeExpr::Reference(reference) if reference.name == "u8")
    )
}

fn borrow_inner_type(ty: &TypeExpr, resolved: &ResolveOutput) -> Option<Type> {
    let scalar = match ty {
        TypeExpr::Reference(reference) if reference.name == "i32" => Some(Type::I32),
        TypeExpr::Reference(reference) if reference.name == "u8" => Some(Type::U8),
        TypeExpr::Reference(reference) if reference.name == "usize" => Some(Type::Usize),
        TypeExpr::Reference(reference) if reference.name == "bool" => Some(Type::Bool),
        _ => None,
    };
    if scalar.is_some() {
        return scalar;
    }

    let value = abi_value_from_type_expr(ty, resolved).ok()?;
    aggregate_type_from_abi_value(&value)
}

fn aggregate_type_from_abi_value(value: &AbiValue) -> Option<Type> {
    if !matches!(value.ty, AbiType::Struct(_)) {
        return None;
    }

    match value.classification {
        ValueClassification::Indirect => Some(Type::Aggregate {
            layout: value.layout,
        }),
        ValueClassification::Direct { words } => Some(Type::DirectAggregate {
            layout: value.layout,
            words,
        }),
    }
}

fn lower_function_body(
    function: &FunctionDecl,
    return_type: &Type,
    root_source: SourceId,
    resolved: &ResolveOutput,
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let success_type = return_type.success_type();
    let statements = function.body.statements.as_slice();

    if statements.is_empty() && *success_type == Type::Void {
        return Ok(vec![success_return_instruction(return_type)]);
    }

    let Some((last, leading)) = statements.split_last() else {
        return Err(unsupported_function_body_diagnostic(&function.name));
    };

    let mut instructions = lower_leading_bindings(leading, context)?;

    match last {
        Stmt::Return(statement) => {
            if let Some(expression) = &statement.expression
                && let Some(return_instructions) =
                    lower_never_return_expression(expression, context)?
            {
                instructions.extend(return_instructions);
                return Ok(instructions);
            }

            if let Some(expression) = &statement.expression
                && matches!(return_type, Type::Fallible(_))
                && let Some(payload) =
                    lower_error_payload(expression, resolved, root_source, Some(context))?
            {
                instructions.extend(lower_fallible_failure(payload));
                return Ok(instructions);
            }

            let return_instructions = match (success_type, &statement.expression) {
                (Type::I32, Some(expression)) => lower_i32_return_expression(expression, context),
                (Type::U8, Some(expression)) => lower_u8_return_expression(expression, context),
                (Type::Usize, Some(expression)) => {
                    lower_usize_return_expression(expression, context)
                }
                (Type::Bool, Some(expression)) => {
                    lower_bool_return_expression(expression, context, "E8007")
                }
                (Type::Str, Some(expression)) => lower_str_return_expression(expression, context),
                (Type::Slice { .. }, Some(expression)) => {
                    lower_slice_return_expression(expression, context)
                }
                (Type::Aggregate { .. } | Type::DirectAggregate { .. }, Some(expression)) => {
                    lower_aggregate_return_expression(
                        expression,
                        success_type,
                        &function.name,
                        resolved,
                        context,
                    )
                }
                (Type::Never, Some(_)) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 can only lower never function `{}` returns from `never` calls",
                        function.name
                    ),
                )]),
                (Type::Void, None) => Ok(vec![Instruction::Return]),
                (Type::Void, Some(_)) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower value returns from void function `{}`",
                        function.name
                    ),
                )]),
                (Type::I32, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from i32 function `{}`",
                        function.name
                    ),
                )]),
                (Type::U8, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from u8 function `{}`",
                        function.name
                    ),
                )]),
                (Type::Usize, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from usize function `{}`",
                        function.name
                    ),
                )]),
                (Type::Bool, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from bool function `{}`",
                        function.name
                    ),
                )]),
                (Type::Str, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from &str function `{}`",
                        function.name
                    ),
                )]),
                (Type::Slice { .. }, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from slice function `{}`",
                        function.name
                    ),
                )]),
                (Type::Aggregate { .. } | Type::DirectAggregate { .. }, None) => {
                    Err(vec![Diagnostic::error(
                        "E8007",
                        format!(
                            "IR v0 cannot lower bare returns from aggregate function `{}`",
                            function.name
                        ),
                    )])
                }
                (Type::Borrow { .. }, _) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower borrow returns from function `{}`",
                        function.name
                    ),
                )]),
                (Type::Never, None) => Err(vec![Diagnostic::error(
                    "E8007",
                    format!(
                        "IR v0 cannot lower bare returns from never function `{}`",
                        function.name
                    ),
                )]),
                (Type::Fallible(_), _) => {
                    unreachable!("fallible success type must be unwrapped")
                }
            }?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                return_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::I32 => {
            let branch_instructions =
                lower_terminal_i32_if_statement(statement, context, "E8007", "functions")?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::If(statement) if success_type == &Type::Bool => {
            let branch_instructions =
                lower_terminal_bool_if_statement(statement, context, "E8007", "functions")?;
            instructions.extend(mark_fallible_success_returns(
                return_type,
                branch_instructions,
            ));
            Ok(instructions)
        }
        Stmt::Expression(statement) => {
            let Some(terminating_instructions) =
                lower_never_return_expression(&statement.expression, context)?
            else {
                if success_type == &Type::Void
                    && let Some(void_instructions) =
                        lower_void_expression_statement(&statement.expression, context)?
                {
                    instructions.extend(void_instructions);
                    instructions.push(success_return_instruction(return_type));
                    return Ok(instructions);
                }

                return Err(unsupported_function_body_diagnostic(&function.name));
            };
            instructions.extend(terminating_instructions);
            Ok(instructions)
        }
        _ => Err(unsupported_function_body_diagnostic(&function.name)),
    }
}

fn lower_leading_bindings(
    statements: &[Stmt],
    context: &mut LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let mut instructions = Vec::new();

    for statement in statements {
        match statement {
            Stmt::Binding(statement) => {
                instructions.extend(lower_local_binding(statement, context)?);
            }
            Stmt::Assignment(statement) => {
                instructions.extend(lower_assignment(statement, context)?);
            }
            Stmt::Expression(statement) => {
                let Some(void_instructions) =
                    lower_void_expression_statement(&statement.expression, context)?
                else {
                    return Err(vec![Diagnostic::error(
                        "E8007",
                        "IR v0 can only lower leading scalar local bindings, scalar assignments, or void call statements before `return`",
                    )]);
                };
                instructions.extend(void_instructions);
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "E8007",
                    "IR v0 can only lower leading scalar local bindings, scalar assignments, or void call statements before `return`",
                )]);
            }
        };
    }

    Ok(instructions)
}

fn lower_aggregate_return_expression(
    expression: &Expr,
    return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    match expression {
        Expr::StructLiteral(literal) => lower_aggregate_struct_literal_return(
            literal,
            return_type,
            function_name,
            resolved,
            context,
        ),
        Expr::Call(call) => lower_aggregate_call_return(call, return_type, function_name, context),
        Expr::Propagate(propagation) => {
            let Expr::Call(call) = unwrap_group(&propagation.expression) else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_fallible_call_return(call, return_type, function_name, context)
        }
        Expr::Identifier(identifier) => {
            lower_aggregate_local_return(&identifier.name, return_type, function_name, context)
        }
        Expr::Member(_) => {
            lower_aggregate_member_return(expression, return_type, function_name, context)
        }
        Expr::Unary(unary) if unary.operator == UnaryOperator::Move => {
            let Expr::Identifier(identifier) = unary.operand.as_ref() else {
                return Err(unsupported_aggregate_return_diagnostic(function_name));
            };
            lower_aggregate_local_return(&identifier.name, return_type, function_name, context)
        }
        Expr::Group(group) => lower_aggregate_return_expression(
            &group.expression,
            return_type,
            function_name,
            resolved,
            context,
        ),
        _ => Err(unsupported_aggregate_return_diagnostic(function_name)),
    }
}

fn unwrap_group(expression: &Expr) -> &Expr {
    match expression {
        Expr::Group(group) => unwrap_group(&group.expression),
        _ => expression,
    }
}

fn lower_aggregate_local_return(
    name: &str,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(return_type);
    let Some((slot_index, layout)) = context.aggregate_slot(name) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if layout != expected_layout || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    Ok(vec![
        Instruction::CopyAggregate {
            destination,
            source: AggregateLocation::Slot(slot_index),
            layout,
        },
        Instruction::Return,
    ])
}

fn lower_aggregate_member_return(
    expression: &Expr,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(return_type);
    let mut temporaries = TemporaryAllocator::new(context)?;
    let access = lower_aggregate_member_field_access(expression, context, &mut temporaries)?
        .ok_or_else(|| unsupported_aggregate_return_diagnostic(function_name))?;
    let source = access.source;
    let source_offset = access.offset;
    let is_copy = access.is_copy;
    let AggregateFieldKind::Aggregate { layout, .. } = access.kind else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if layout != expected_layout || !is_copy || !supported_aggregate_copy_layout(layout) {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let mut instructions = access.instructions;
    instructions.push(Instruction::CopyAggregateRange {
        destination,
        destination_offset: 0,
        source,
        source_offset,
        layout,
    });
    instructions.push(Instruction::Return);
    Ok(instructions)
}

fn lower_aggregate_fallible_call_return(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let target = context.call_target(call, &identifier.name);
    let Some(Type::Fallible(success_type)) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if success_type.as_ref() != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallFallibleAggregate {
                destination: AggregateLocation::Return,
                target,
                arguments,
                failure_mode: FallibleFailureMode::Propagate,
            });
        }
        Type::DirectAggregate { layout, .. } => {
            instructions.push(Instruction::CallFallibleDirectAggregate {
                destination: AggregateLocation::DirectReturn,
                target,
                arguments,
                layout: *layout,
                failure_mode: FallibleFailureMode::Propagate,
            });
        }
        _ => unreachable!("fallible aggregate call return lowering requires aggregate return type"),
    }
    instructions.push(Instruction::ReturnFallibleSuccess);
    Ok(instructions)
}

fn lower_aggregate_call_return(
    call: &crate::ast::CallExpr,
    return_type: &Type,
    function_name: &str,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Expr::Identifier(identifier) = call.callee.as_ref() else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    let target = context.call_target(call, &identifier.name);
    let Some(callee_return_type) = context.call_return_type(&target) else {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    };
    if callee_return_type != return_type {
        return Err(unsupported_aggregate_return_diagnostic(function_name));
    }

    let (mut instructions, arguments) =
        lower_call_arguments_to_scalar_arguments(call, &target, &identifier.name, context)?;
    match return_type {
        Type::Aggregate { .. } => {
            instructions.push(Instruction::CallAggregate {
                destination: AggregateLocation::Return,
                target,
                arguments,
            });
        }
        Type::DirectAggregate { layout, .. } => {
            instructions.push(Instruction::CallDirectAggregate {
                destination: AggregateLocation::DirectReturn,
                target,
                arguments,
                layout: *layout,
            });
        }
        _ => unreachable!("aggregate call return lowering requires aggregate return type"),
    }
    instructions.push(Instruction::Return);
    Ok(instructions)
}

fn lower_aggregate_struct_literal_return(
    literal: &StructLiteralExpr,
    return_type: &Type,
    function_name: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let (expected_layout, destination) = aggregate_return_layout_and_destination(return_type);

    let subject = format!("returns from function `{function_name}`");
    let lowered_direct = lower_aggregate_struct_literal_to_location(
        literal,
        expected_layout,
        destination,
        "E8007",
        &subject,
        resolved,
        context,
    );
    let mut instructions = match lowered_direct {
        Ok(instructions) => instructions,
        Err(error) if matches!(destination, AggregateLocation::DirectReturn) => {
            lower_direct_aggregate_struct_literal_return_through_slot(
                literal,
                expected_layout,
                &subject,
                resolved,
                context,
            )
            .map_err(|_| error)?
        }
        Err(error) => return Err(error),
    };
    instructions.push(Instruction::Return);
    Ok(instructions)
}

fn lower_direct_aggregate_struct_literal_return_through_slot(
    literal: &StructLiteralExpr,
    expected_layout: crate::abi::ValueLayout,
    subject: &str,
    resolved: &ResolveOutput,
    context: &LoweringContext,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    if !supported_aggregate_copy_layout(expected_layout) {
        return Err(unsupported_aggregate_return_diagnostic(
            context.function_name(),
        ));
    }

    let mut temporaries = TemporaryAllocator::new(context)?;
    let slot_index = temporaries.next_aggregate_slot();
    let mut instructions = vec![Instruction::ReserveAggregateSlot {
        slot_index,
        layout: expected_layout,
    }];
    instructions.extend(lower_aggregate_struct_literal_to_location_with_temporaries(
        literal,
        expected_layout,
        AggregateLocation::Slot(slot_index),
        "E8007",
        subject,
        resolved,
        context,
        &mut temporaries,
    )?);
    instructions.push(Instruction::CopyAggregate {
        destination: AggregateLocation::DirectReturn,
        source: AggregateLocation::Slot(slot_index),
        layout: expected_layout,
    });
    Ok(instructions)
}

fn aggregate_return_layout_and_destination(
    return_type: &Type,
) -> (crate::abi::ValueLayout, AggregateLocation) {
    match return_type {
        Type::Aggregate { layout } => (*layout, AggregateLocation::Return),
        Type::DirectAggregate { layout, .. } => (*layout, AggregateLocation::DirectReturn),
        _ => unreachable!("aggregate return lowering requires aggregate return type"),
    }
}

fn unsupported_aggregate_return_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower aggregate returns from function `{function_name}` from a supported struct literal, an aggregate call, or a supported aggregate local slot"
        ),
    )]
}

fn lower_fallible_failure(payload: ErrorPayload) -> Vec<Instruction> {
    let (code, message) = payload.into_str_values();
    vec![Instruction::ReturnFallibleFailure { code, message }]
}

fn unsupported_function_body_diagnostic(function_name: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8007",
        format!(
            "IR v0 can only lower function `{function_name}` bodies containing leading scalar local bindings, scalar assignments, or void call statements followed by `return`"
        ),
    )]
}
