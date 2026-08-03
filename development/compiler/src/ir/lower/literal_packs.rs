//! Lowering for the compiler-owned, non-escaping sequence literal element pack.

use super::bindings::lower_local_binding;
use super::context::{LiteralPackLoweringSegment, LoweringContext};
use super::control_flow::nonterminal::lower_nonterminal_region_body;
use super::functions::{lower_scope_end_drops_for_locals_since, mark_explicit_moves_in_expression};
use crate::ast::{
    BindingKind, BindingStmt, CollectionForStmt, Expr, IdentifierExpr, LiteralPackForStmt,
    UnaryExpr, UnaryOperator,
};
use crate::diagnostics::Diagnostic;
use crate::ir::Instruction;
use crate::source::SourceMap;

pub(in crate::ir::lower) fn lower_literal_pack_for_statement(
    statement: &LiteralPackForStmt,
    context: &mut LoweringContext,
    diagnostic_code: &'static str,
    subject: &str,
    sources: &SourceMap,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let Some(pack) = context.literal_pack(&statement.pack_name).cloned() else {
        return Err(vec![Diagnostic::error(
            diagnostic_code,
            format!("IR cannot lower literal pack iteration in {subject}"),
        )]);
    };
    let mut instructions = Vec::new();

    for (index, segment) in pack.segments.iter().enumerate() {
        if let LiteralPackLoweringSegment::Spread {
            iterator_parameter_name,
            plan,
            ..
        } = segment
        {
            let synthetic = CollectionForStmt {
                span: statement.span,
                name: statement.name.clone(),
                name_span: statement.name_span,
                source: Expr::Identifier(IdentifierExpr {
                    span: statement.pack_span,
                    name: iterator_parameter_name.clone(),
                }),
                body: statement.body.clone(),
            };
            let iteration_plan = crate::typecheck::TypecheckCollectionForPlan {
                binding_span: statement.name_span,
                source_span: statement.pack_span,
                source_mode: crate::typecheck::TypecheckCollectionForSourceMode::Direct,
                source_type: plan.iterator_type.clone(),
                iterator_type: plan.iterator_type.clone(),
                item_type: plan.iterator_item_type.clone(),
                conversion: None,
                step: plan.step.clone(),
            };
            let projected_item_type = (plan.mode
                == crate::typecheck::TypecheckSequenceSpreadMode::Copy)
                .then_some(&plan.pack_item_type);
            instructions.extend(super::collection_for::lower_literal_pack_spread_with_plan(
                &synthetic,
                &iteration_plan,
                projected_item_type,
                index,
                context,
                diagnostic_code,
                subject,
                sources,
            )?);
            continue;
        }
        let LiteralPackLoweringSegment::Value {
            parameter_name: element_name,
        } = segment
        else {
            unreachable!()
        };
        let local_mark = context.local_mark();
        let element = Expr::Identifier(IdentifierExpr {
            span: statement.pack_span,
            name: element_name.clone(),
        });
        let initializer = if context.aggregate_local(element_name).is_some() {
            Expr::Unary(UnaryExpr {
                span: statement.pack_span,
                operator: UnaryOperator::Move,
                operator_span: statement.pack_span,
                operand: Box::new(element),
            })
        } else {
            element
        };
        let binding = BindingStmt {
            span: statement.span,
            kind: BindingKind::Let,
            name: statement.name.clone(),
            name_span: statement.name_span,
            ty: Some(pack.element_type.clone()),
            initializer,
        };
        instructions.extend(lower_local_binding(&binding, context)?);
        mark_explicit_moves_in_expression(&binding.initializer, context);

        let lowered = lower_nonterminal_region_body(
            &statement.body,
            context,
            local_mark,
            None,
            &[],
            diagnostic_code,
            subject,
            sources,
        )?;
        instructions.extend(lowered.instructions);
        if lowered.ends_execution {
            break;
        }
        instructions.extend(lower_scope_end_drops_for_locals_since(context, local_mark)?);
        let hidden_name = format!(
            "<literal-loop:{}:{}:{index}>",
            statement.name_span.start, statement.name_span.end
        );
        if !context.rename_local(&statement.name, hidden_name) {
            return Err(vec![Diagnostic::error(
                diagnostic_code,
                "IR cannot retire a literal pack iteration binding",
            )]);
        }
    }
    Ok(instructions)
}
