//! Source-backed result closure, staging, proved Copy and dependency-safe reuse.
use super::support::{source_case, steps};
use super::*;
use crate::value_access::{BoundaryAction, ExpressionKind, Query as TypedQuery};
use nocter_declarations::value_access::PayloadSchema;
use nocter_model::ValueFunctionOrigin as Origin;

#[test]
fn result_relations_close_before_body_resource_acceptance() {
    let text = source_case("a99-callable-result-forward.nct");
    let (sources, tree) = input(&text);
    let headers = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let context = headers.bind(bound(&sources, &tree)).unwrap();
    let run = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let pending = TypedQuery::prepare(&context, run).unwrap();
    assert!(!pending.results_closed());
    assert!(matches!(
        flow::Query::prepare(Arc::new(pending)),
        Err(flow::IntegrityError::UnclosedResultWitnesses)
    ));
    let unit = Unit::prepare(&context).unwrap();
    let increment = unit
        .program()
        .resolve(unit.program().symbols().get("increment").unwrap())
        .unwrap();
    for name in ["make", "forward"] {
        let query = named(&unit, name);
        let witness = unit
            .result_witness(query.typed().source_owner().into())
            .unwrap();
        assert_eq!(witness.origin, Origin::Declaration(increment));
        assert!(query.typed().results_closed());
        assert_eq!(
            query.typed().result_data(),
            unit.program().function_data(witness).unwrap()
        );
    }
    for (_, query) in unit.bodies() {
        for (_, expression) in query.typed().body().expressions().iter() {
            if let Some(access) = expression.ty.access() {
                assert!(
                    !matches!(unit.program().data_schema(access.data()), Some(PayloadSchema::Function(w)) if matches!(w.origin, Origin::Result(_)))
                );
            }
            if let ExpressionKind::Call {
                target, arguments, ..
            } = &expression.kind
            {
                assert!(!matches!(target, Origin::Result(_)));
                assert!(
                    arguments
                        .iter()
                        .filter_map(|a| a.binding.as_ref())
                        .all(|b| !matches!(b.witness.origin, Origin::Result(_)))
                );
            }
        }
    }
}

#[test]
fn result_environment_is_staged_before_automatic_input_cleanup() {
    let unit = accepted(&source_case("a117-callable-result-staged-cleanup.nct"));
    let trace = steps(named(&unit, "make"));
    let staged = trace.iter().position(|step| matches!(step, Step::Move { destination, .. } if destination.holder() == Holder::Return)).unwrap();
    let cleaned = trace.iter().position(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)).unwrap();
    assert!(staged < cleaned);
    assert_eq!(trace.iter().filter(|step| matches!(step, Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError)).count(), 1);
    assert!(!trace.iter().any(
        |step| matches!(step, Step::Dispose { source, .. } if source.holder() == Holder::Return)
    ));
}

#[test]
fn exact_result_schema_discharges_copy_supply_without_moving_the_original() {
    let unit = accepted(&source_case("a106-callable-result-copy.nct"));
    let query = named(&unit, "run");
    assert!(
        query
            .typed()
            .body()
            .expressions()
            .iter()
            .any(|(_, expression)| matches!(
                expression.kind,
                ExpressionKind::Boundary {
                    action: BoundaryAction::CopyPayload,
                    ..
                }
            ))
    );
    for (_, local) in query.typed().body().locals().iter() {
        assert_eq!(unit.program().data_copyable(local.data), Some(true));
    }
    assert!(query.initialization().violations().is_empty());
    assert!(query.access().violations().is_empty());
}

#[test]
fn producer_body_changes_invalidate_closed_queries_even_when_their_own_body_is_equal() {
    let original = "func increment(value: i32): i32 { value + 1 }\nfunc decrement(value: i32): i32 { value - 1 }\nfunc make(): func(i32): i32 { increment }\nfunc run(): i32 { make()(1) }";
    let (a, ta) = input(original);
    let headers = declarations::Query::prepare(bound(&a, &ta)).unwrap();
    let context = headers.bind(bound(&a, &ta)).unwrap();
    let run = context
        .program()
        .resolve(context.program().symbols().get("run").unwrap())
        .unwrap();
    let pending = TypedQuery::prepare(&context, run).unwrap();
    let unit = Unit::prepare(&context).unwrap();
    assert!(unit.body(run).unwrap().typed().bind(&context).is_ok());
    let changed = original.replace("{ increment }", "{ decrement }");
    let (b, tb) = input(&changed);
    let current = headers.bind(bound(&b, &tb)).unwrap();
    assert!(pending.bind(&current).is_ok());
    assert!(matches!(
        unit.body(run).unwrap().typed().bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
    assert!(matches!(
        unit.bind(&current),
        Err(crate::value_access::Failure::ChangedBody)
    ));
}

#[test]
fn recursive_result_proofs_and_explicit_returns_share_one_named_producer() {
    for case in [
        "a102-callable-result-recursive-anchor.nct",
        "a103-callable-result-mutual-anchor.nct",
        "a114-callable-result-loop-return.nct",
    ] {
        let unit = accepted(&source_case(case));
        let increment = unit
            .program()
            .resolve(unit.program().symbols().get("increment").unwrap())
            .unwrap();
        for (id, declaration) in unit.program().codes().iter() {
            if !declaration.result_contracts().is_empty() {
                assert_eq!(
                    unit.result_witness(id.into()).unwrap().origin,
                    Origin::Declaration(increment)
                );
            }
        }
    }
}
