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

#[test]
fn contextual_indexed_replacement_preserves_containing_roots_and_sibling_views() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn contextual_indexed_receipts_share_original_numeric_facts_with_ownership() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-same-context.nct"
    ));
    assert_eq!(answer(source, "ANSWER"), Value::integer(42));
}

#[test]
fn unique_indexed_contexts_do_not_elide_bounds_or_choose_a_multi_element_context() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    for (changed, expected) in [
        (
            source.replace("calculate(0)", "calculate(1)"),
            crate::value_access::evaluation::Rule::Execution(
                nocter_constant_evaluation::state::Rule::IndexOutOfBounds,
            ),
        ),
        (
            source.replace(
                "[wrap(addition(20))]",
                "[wrap(addition(20)), wrap(addition(20))]",
            ),
            crate::value_access::evaluation::Rule::UnresolvedContext,
        ),
    ] {
        assert_ne!(changed, source);
        let (sources, tree) = input(&changed);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Evaluation { diagnostic, .. }) if diagnostic.rule() == expected),
            "{failure:?}"
        );
    }
}

#[test]
fn a_unique_coordinate_does_not_allow_overwriting_a_still_used_element_reference() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-indexed-replacement.nct"
    ));
    let changed = source
        .replace(
            "let first = pair.0[index](20)",
            "read held = pair.0[index]\n    let first = held(20)",
        )
        .replace(
            "let second = pair.0[index](8)",
            "let _ = held(20)\n    let second = pair.0[index](8)",
        );
    assert_ne!(changed, source);
    let (sources, tree) = input(&changed);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Flow { .. })),
        "{failure:?}"
    );
}
