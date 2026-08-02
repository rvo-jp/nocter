//! Common typed-literal call lowering for every aggregate destination context.

use super::allocation_contexts::lower_allocation_context_override;
use super::context::LoweringContext;
use super::expressions::{TemporaryAllocator, lower_call_arguments_with_explicit_types};
use crate::ast::{
    BorrowType, CallExpr, Expr, IdentifierExpr, LiteralShape, TypeExpr, TypeReference,
};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, Instruction};

pub(in crate::ir::lower) fn lower_typed_literal_to_location(
    expression: &Expr,
    destination: AggregateLocation,
    context: &LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let (span, shape, arguments, target_span, using) = match unwrap_group(expression) {
        Expr::TypedSequenceLiteral(literal) => (
            literal.span,
            LiteralShape::Sequence,
            literal.elements.clone(),
            literal.target.span(),
            literal
                .using
                .as_ref()
                .map(|using| using.allocator.as_ref().clone()),
        ),
        Expr::TypedStringLiteral(literal) => (
            literal.span,
            LiteralShape::String,
            vec![Expr::StringLiteral(literal.text.clone())],
            literal.target.span(),
            literal
                .using
                .as_ref()
                .map(|using| using.allocator.as_ref().clone()),
        ),
        _ => return Ok(None),
    };
    let Some((target, target_name)) =
        context.typed_literal_call_target(span, shape, arguments.len())
    else {
        return Err(literal_lowering_diagnostic(
            "the hidden callable target is unavailable",
        ));
    };
    let return_type = context.call_return_type(&target).cloned().ok_or_else(|| {
        literal_lowering_diagnostic("the hidden callable result ABI is unavailable")
    })?;
    let parameter_types = match shape {
        LiteralShape::Sequence => arguments
            .iter()
            .map(|argument| context.expression_type_expr(argument.span()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| literal_lowering_diagnostic("an element has no concrete type fact"))?,
        LiteralShape::String => vec![readonly_str_type(target_span)],
    };
    let synthetic_call = CallExpr {
        span,
        callee: Box::new(Expr::Identifier(IdentifierExpr {
            span: target_span,
            name: target_name.clone(),
        })),
        arguments_span: span,
        arguments,
    };

    let mut local_context = context.clone();
    let override_ = using
        .as_ref()
        .map(|allocator| lower_allocation_context_override(allocator, &mut local_context))
        .transpose()?;
    let lowering_context = override_
        .as_ref()
        .map_or(&local_context, |override_| &override_.context);
    let mut temporaries = TemporaryAllocator::new(lowering_context)?;
    let (mut argument_instructions, arguments) = lower_call_arguments_with_explicit_types(
        &synthetic_call,
        &target,
        &target_name,
        lowering_context,
        &mut temporaries,
        Some(&parameter_types),
    )?;
    let mut instructions = Vec::new();
    if let Some(override_) = &override_ {
        instructions.extend(override_.enter.iter().cloned());
    }
    instructions.append(&mut argument_instructions);
    super::aggregates::push_aggregate_call_instruction(
        &mut instructions,
        &return_type,
        destination,
        target,
        arguments,
        super::aggregates::aggregate_type_layout(&return_type)
            .ok_or_else(|| literal_lowering_diagnostic("the literal result is not an aggregate"))?,
    );
    if let Some(override_) = override_ {
        instructions.push(override_.restore);
    }
    Ok(Some(instructions))
}

fn readonly_str_type(span: crate::source::ByteSpan) -> TypeExpr {
    TypeExpr::Borrow(BorrowType {
        span,
        is_readwrite: false,
        inner: Box::new(TypeExpr::Reference(TypeReference {
            span,
            name: "str".to_string(),
        })),
    })
}

fn unwrap_group(mut expression: &Expr) -> &Expr {
    while let Expr::Group(group) = expression {
        expression = &group.expression;
    }
    expression
}

fn literal_lowering_diagnostic(detail: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "E8014",
        format!("IR cannot lower typed literal construction: {detail}"),
    )]
}
