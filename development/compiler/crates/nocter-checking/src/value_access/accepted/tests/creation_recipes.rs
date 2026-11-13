//! Source creation snapshots survive code/type normalization without becoming payload receipts.
use super::*;
use crate::value_access::{CreationRecipe, ExpressionKind};
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{ValueApplication, ValueCode, ValueField, ValueFunctionOrigin as Origin};
use std::collections::BTreeSet;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/capture-producers.nct"
));
fn code(unit: &Unit, name: &str) -> AccessCodeId {
    unit.program()
        .resolve_code(unit.program().symbols().get(name).unwrap())
        .unwrap()
}
fn literal(unit: &Unit, owner: ValueCode) -> (nocter_model::ValueClosure, &CreationRecipe) {
    let body = unit.code_body(owner).unwrap().typed().body();
    let (expression, closure, capture) = body
        .expressions()
        .iter()
        .find_map(|(id, value)| {
            if let ExpressionKind::Closure { closure, capture } = value.kind {
                Some((id, closure, capture))
            } else {
                None
            }
        })
        .unwrap();
    let recipe = unit.creation_recipe(closure).unwrap();
    assert_eq!(recipe.owner, owner);
    assert_eq!(recipe.expression, expression);
    let capture = body
        .expressions()
        .get(capture)
        .unwrap()
        .ty
        .access()
        .unwrap()
        .data();
    assert_eq!(
        unit.program().closure(closure).unwrap().environment(),
        capture
    );
    (closure, recipe)
}
fn applications(unit: &Unit, name: &str) -> Vec<ValueApplication> {
    let owner = code(unit, name).into();
    let maker = Origin::Declaration(code(unit, "make"));
    unit.code_body(owner)
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .filter_map(|(call, expression)| {
            matches!(&expression.kind, ExpressionKind::Call { target, .. } if *target == maker)
                .then_some(ValueApplication { owner, call })
        })
        .collect()
}
fn producer(application: ValueApplication) -> FunctionWitness {
    FunctionWitness {
        origin: Origin::Application(application),
        invocation: nocter_model::ValueInvocation::Read,
    }
}
#[test]
fn creation_preserves_actual_component_producers_before_later_updates_and_type_projection() {
    let unit = accepted(SOURCE);
    let (closure, recipe) = literal(&unit, code(&unit, "captured_components").into());
    let creations = applications(&unit, "captured_components");
    assert_eq!(creations.len(), 4);
    assert_ne!(
        recipe.environment,
        unit.program().closure(closure).unwrap().environment()
    );
    for (field, application) in [(0, creations[2]), (1, creations[0])] {
        assert_eq!(
            recipe
                .producers
                .producers_at(&nocter_model::FieldPath::new([ValueField::Tuple(field)])),
            &BTreeSet::from([producer(application)])
        );
    }
    let answer = unit
        .program()
        .resolve_constant(unit.program().symbols().get("ANSWER").unwrap())
        .unwrap();
    assert_eq!(
        unit.constant_value(answer),
        Some(&nocter_constant_evaluation::state::Value::integer(42).freeze())
    );
}
#[test]
fn nested_creation_records_its_exact_lexical_code_and_dead_formation_has_no_recipe() {
    let unit = accepted(SOURCE);
    let (outer, _) = literal(&unit, code(&unit, "nested_creation").into());
    let (inner, recipe) = literal(&unit, ValueCode::Closure(outer));
    assert_eq!(inner.owner, outer.owner);
    assert_eq!(recipe.owner, ValueCode::Closure(outer));
    let body = named(&unit, "dormant_creation").typed().body();
    let closure = body
        .expressions()
        .iter()
        .find_map(|(_, expression)| {
            if let ExpressionKind::Closure { closure, .. } = expression.kind {
                Some(closure)
            } else {
                None
            }
        })
        .unwrap();
    assert!(unit.creation_recipe(closure).is_none());
}
#[test]
fn repeated_source_formation_joins_possible_inputs_without_choosing_one_iteration() {
    let unit = accepted(SOURCE);
    let (_, recipe) = literal(&unit, code(&unit, "repeated_creation").into());
    let expected = applications(&unit, "repeated_creation")
        .into_iter()
        .map(producer)
        .collect::<BTreeSet<_>>();
    assert_eq!(expected.len(), 2);
    assert_eq!(
        recipe
            .producers
            .producers_at(&nocter_model::FieldPath::default()),
        &expected
    );
}

#[test]
fn source_rebinding_cannot_reuse_a_creation_snapshot_after_same_typed_capture_effects_change() {
    let (sources, tree) = input(SOURCE);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Query::prepare(syntax).unwrap();
    let context = headers.bind(syntax).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    unit.bind(&context).unwrap();
    let changed = SOURCE.replace("alias.0 = make(44)", "alias.1 = make(44)");
    let (sources, tree) = input(&changed);
    let current = headers.bind(bound(&sources, &tree)).unwrap();
    assert!(matches!(
        unit.bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    let fresh = Unit::prepare(&current).unwrap();
    let (_, recipe) = literal(&fresh, code(&fresh, "captured_components").into());
    let creations = applications(&fresh, "captured_components");
    for (field, application) in [(0, creations[1]), (1, creations[2])] {
        assert_eq!(
            recipe
                .producers
                .producers_at(&nocter_model::FieldPath::new([ValueField::Tuple(field)])),
            &BTreeSet::from([producer(application)])
        );
    }
}
