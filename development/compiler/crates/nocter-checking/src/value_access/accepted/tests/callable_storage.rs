//! Ordinary callable storage keeps exact contexts through retirement, receipt and aliases.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value;

#[test]
fn contextual_callable_replacement_preserves_retired_received_and_transferred_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}
#[test]
fn contextual_replacement_cannot_overwrite_a_still_used_reference() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-callable-replacement.nct"
    ));
    let changed = source.replace(
        "action = wrap(subtraction(50))",
        "read retained = action\n    action = wrap(subtraction(50))\n    let _ = retained(0)",
    );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
        "{failure:?}"
    );
}

#[test]
fn contextual_component_replacement_preserves_sibling_views_and_saved_instances() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-component-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}
