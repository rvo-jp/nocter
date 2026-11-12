//! Result witnesses close before structural payload and Copy obligations are admitted.
use super::initializers::answer;
use super::*;
use nocter_constant_evaluation::state::Value as Datum;
use nocter_declarations::value_access::PayloadReadiness;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/native/value-access/structural-result-values.nct"
));

#[test]
fn units_without_bodies_retain_their_canonical_type_authority() {
    for source in ["", "struct Item { value: i32 }"] {
        let unit = accepted(source);
        assert_eq!(unit.bodies().count(), 0);
        assert!(unit.program().resolve_body_types(&BTreeMap::new()).is_ok());
    }
}

#[test]
fn provisional_structures_do_not_admit_copy_or_executable_resources() {
    let (sources, tree) = input(SOURCE);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let id = context
        .program()
        .resolve(context.program().symbols().get("arrays").unwrap())
        .unwrap();
    let raw = crate::value_access::Query::prepare(&context, id).unwrap();
    assert!(!raw.results_closed());
    assert!(
        raw.body()
            .locals()
            .iter()
            .any(|(_, local)| raw.program().payload_readiness(local.data)
                == PayloadReadiness::Pending)
    );
    assert!(matches!(
        flow::Query::prepare(Arc::new(raw.clone())),
        Err(flow::IntegrityError::UnclosedResultWitnesses)
    ));
    let unit = Unit::prepare(&context).unwrap();
    let edited = SOURCE.replace("amount + value", "amount - value");
    let (changed, changed_tree) = input(&edited);
    let current = headers.bind(bound(&changed, &changed_tree)).unwrap();
    assert!(raw.bind(&current).is_ok());
    assert!(matches!(
        unit.body(id).unwrap().typed().bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
}

#[test]
fn structural_results_share_one_closed_authority_in_required_execution() {
    assert_eq!(answer(SOURCE, "ANSWER"), Datum::integer(42));
    let unit = accepted(SOURCE);
    for (_, body) in unit.bodies() {
        assert!(std::ptr::eq(body.typed().program(), unit.program()));
        assert!(body.typed().results_closed());
        for (_, local) in body.typed().body().locals().iter() {
            assert!(
                unit.program().payload_readiness(local.data) == PayloadReadiness::Closed
                    || unit
                        .data_evidence(local.data)
                        .is_ok_and(|evidence| evidence.payload().is_none())
                    || matches!(unit.program().data_schema(local.data), Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) if matches!(witness.origin, nocter_model::ValueFunctionOrigin::Parameter(_)))
            );
        }
    }
}

#[test]
fn structural_projections_do_not_anchor_unproduced_recursion_or_hide_retirement() {
    let source = "func ghost(): func(i32): i32 { let callbacks = [ghost()]\ncallbacks[0] }";
    let (sources, tree) = input(source);
    let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
    assert!(
        matches!(failure, SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == crate::value_access::Rule::TypeMismatch)
    );
    let source = format!(
        "{SOURCE}\nfunc retired(): func(i32): i32 {{ let callbacks = [make(2)]\ndrop callbacks\ncallbacks[0] }}"
    );
    let (sources, tree) = input(&source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}

#[test]
fn result_resolved_structural_capture_has_an_actual_owned_environment() {
    let source = format!(
        "{SOURCE}\nfunc pending_capture(): void {{ let callbacks = [make(2)]\nlet action = (callbacks;): i32 {{ callbacks[0](40) }} }}"
    );
    let (sources, tree) = input(&source);
    SourceUnit::prepare(bound(&sources, &tree)).unwrap();
}

#[test]
fn a_closed_selected_result_witness_does_not_elide_its_producing_bounds_check() {
    let source = format!("{SOURCE}\nconst BAD: i32 = selected_array(2)(0)");
    let (sources, tree) = input(&source);
    assert!(
        matches!(SourceUnit::prepare(bound(&sources, &tree)), Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) if diagnostic.rule() == crate::value_access::evaluation::Rule::Execution(nocter_constant_evaluation::state::Rule::IndexOutOfBounds))
    );
}

#[test]
fn structural_results_reject_different_witnesses_and_unproved_duplication_locally() {
    for (suffix, rule) in [
        (
            "func probe(): void { let values = [make(1), named()] }",
            crate::value_access::Rule::TypeMismatch,
        ),
        (
            "func probe(): void { let values = [[make(1)], [named()]] }",
            crate::value_access::Rule::TypeMismatch,
        ),
        (
            "func probe(): void { let values = [owned(1); 2] }",
            crate::value_access::Rule::InvalidConstruction,
        ),
        (
            "func probe(): void { let values = [owned(1)]\nlet copied = values }",
            crate::value_access::Rule::MissingTransfer,
        ),
        (
            "func probe(): void { let values = (owned(1), 0)\nlet copied = values }",
            crate::value_access::Rule::MissingTransfer,
        ),
        (
            "func probe(): i32 { [owned(42)][0]() }",
            crate::value_access::Rule::UnsupportedData,
        ),
    ] {
        let source = format!("{SOURCE}\n{suffix}");
        let (sources, tree) = input(&source);
        let failure = SourceUnit::prepare(bound(&sources, &tree)).unwrap_err();
        assert!(
            matches!(&failure, SourceFailure::Checking(Failure::Body { failure: crate::value_access::Failure::Rejected(error), .. }) if error.rule() == rule),
            "{suffix}: {failure:?}"
        );
    }
}

#[test]
fn explicit_structural_result_retirement_still_retires_copy_values() {
    let source = format!(
        "{SOURCE}\nfunc probe(): void {{ let values = [make(1)]\nlet moved = values^\nlet invalid = values }}"
    );
    let (sources, tree) = input(&source);
    assert!(matches!(
        SourceUnit::prepare(bound(&sources, &tree)),
        Err(SourceFailure::Checking(Failure::Flow { .. }))
    ));
}
