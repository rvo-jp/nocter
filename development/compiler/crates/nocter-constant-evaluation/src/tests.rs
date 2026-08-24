use std::collections::HashMap;

use nocter_model::{ArenaBuilder, BuiltinType, ConstantId, ConstantValue};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{NodeId, NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

use crate::{
    ConstantEvaluationRule, ConstantPlanError, ConstantPlanRule, ConstantReference,
    ConstantResolver, ConstantScalarType, evaluate_constant_plans, evaluate_expression_plan,
    plan_expression,
};

struct Resolver {
    reference: Option<ConstantReference>,
    conversion: Option<ConstantScalarType>,
}

impl ConstantResolver for Resolver {
    type Error = ();

    fn resolve_constant(&mut self, _node: NodeId) -> Result<ConstantReference, Self::Error> {
        self.reference.ok_or(())
    }

    fn resolve_type(&mut self, _node: NodeId) -> Result<Option<ConstantScalarType>, Self::Error> {
        Ok(self.conversion)
    }
}

#[test]
fn signed_minimum_literal_is_evaluated_in_the_signed_result_domain() {
    let (sources, tree, expression) = parsed_expression("-128");
    let mut resolver = Resolver {
        reference: None,
        conversion: None,
    };
    let plan = plan_expression(
        sources.get(tree.source()).unwrap(),
        &tree,
        expression,
        ConstantScalarType::Integer(BuiltinType::I8),
        &mut resolver,
    )
    .unwrap();

    assert_eq!(
        evaluate_expression_plan(&plan, |_| None).unwrap(),
        ConstantValue::Integer(-128)
    );
}

#[test]
fn short_circuiting_skips_values_but_not_rhs_type_planning() {
    let (sources, tree, expression) = parsed_expression("false && (1 / 0 == 0)");
    let mut resolver = Resolver {
        reference: None,
        conversion: None,
    };
    let plan = plan_expression(
        sources.get(tree.source()).unwrap(),
        &tree,
        expression,
        ConstantScalarType::Bool,
        &mut resolver,
    )
    .unwrap();
    assert_eq!(
        evaluate_expression_plan(&plan, |_| None).unwrap(),
        ConstantValue::Bool(false)
    );

    let (sources, tree, expression) = parsed_expression("false && 1");
    let error = plan_expression(
        sources.get(tree.source()).unwrap(),
        &tree,
        expression,
        ConstantScalarType::Bool,
        &mut resolver,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ConstantPlanError::Rule {
            rule: ConstantPlanRule::TypeMismatch,
            ..
        }
    ));
}

#[test]
fn authored_dependency_cycles_are_rejected_before_evaluation() {
    let mut ids = ArenaBuilder::<ConstantId, ()>::new();
    let first = ids.insert(());
    let second = ids.insert(());
    let (first_sources, first_tree, first_expression) = parsed_expression("second");
    let (second_sources, second_tree, second_expression) = parsed_expression("first");
    let expected = ConstantScalarType::Integer(BuiltinType::I32);
    let mut first_resolver = Resolver {
        reference: Some(ConstantReference::new(second, expected)),
        conversion: None,
    };
    let mut second_resolver = Resolver {
        reference: Some(ConstantReference::new(first, expected)),
        conversion: None,
    };
    let first_plan = plan_expression(
        first_sources.get(first_tree.source()).unwrap(),
        &first_tree,
        first_expression,
        expected,
        &mut first_resolver,
    )
    .unwrap();
    let second_plan = plan_expression(
        second_sources.get(second_tree.source()).unwrap(),
        &second_tree,
        second_expression,
        expected,
        &mut second_resolver,
    )
    .unwrap();
    let plans = HashMap::from([(first, first_plan), (second, second_plan)]);

    assert_eq!(
        evaluate_constant_plans(&plans).unwrap_err().rule(),
        ConstantEvaluationRule::DependencyCycle
    );
}

fn parsed_expression(text: &str) -> (SourceMap, SyntaxTree, NodeId) {
    let mut sources = SourceMap::new();
    let source = sources
        .add_bytes(
            SourceName::new("/constant.nct"),
            format!("const value: i32 = {text}\n").as_bytes(),
        )
        .unwrap();
    let tree = parse(sources.get(source).unwrap(), ParseGoal::SourceFile);
    assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
    let expression = descendants(&tree, tree.root_id())
        .into_iter()
        .find(|node| {
            tree.node(*node)
                .is_some_and(|node| node.kind() == NodeKind::Expression)
        })
        .unwrap();
    (sources, tree, expression)
}

fn descendants(tree: &SyntaxTree, root: NodeId) -> Vec<NodeId> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        result.push(node);
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    result
}
