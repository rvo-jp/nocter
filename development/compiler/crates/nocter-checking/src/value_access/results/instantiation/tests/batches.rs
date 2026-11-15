//! A metadata batch shares solved dependencies without merging original occurrences or failures.
use super::*;

#[test]
fn source_data_batches_keep_contexts_charge_hits_and_discard_interrupted_work() {
    use crate::value_access::{ExpressionKind, ValueSource};
    let fixture = nocter_test_support::ValueSourceFixture::new(
        "func identity(value: i32): i32 { value }\nfunc addition(number: i32): func(i32): i32 { (number; value: i32) { number + value } }\nfunc wrap(take action: func(i32): i32): func(i32): i32 { (action^; value: i32) { action(value) } }\nfunc run(): void { let first = wrap(identity)\nlet second = wrap(addition(20))\ndrop first\ndrop second }",
    );
    let unit = crate::value_access::accepted::SourceUnit::prepare(fixture.syntax())
        .unwrap()
        .into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let body = unit.bodies().find(|(id, _)| *id == code("run")).unwrap().1;
    let evidence = body.typed().result_evidence.as_ref().unwrap();
    let frame = program.bind_code(code("run").into(), []).unwrap();
    let roots = evidence.value_sources(frame.code()).unwrap().nodes().filter_map(|(source, node)| {
        let ValueSource::CallResult(site) = node.value else { return None };
        let expression = body.typed().body().expressions().get(site.call)?;
        matches!(expression.kind, ExpressionKind::Call { target: Origin::Declaration(target), .. } if target == code("wrap"))
            .then_some((source, node.data.unwrap()))
    }).collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);
    let budget = |steps| WorkBudget::new(NonZeroU64::new(steps).unwrap());
    let mut work = budget(100_000);
    let mut query = Query::new(program, evidence, &mut work);
    let first = query.data(&frame, roots[0].0, roots[0].1).unwrap();
    let second = query.data(&frame, roots[1].0, roots[1].1).unwrap();
    assert!(!first.has_same_representation(program, &second));
    let entries = query.entries.len();
    *query.work = budget(1);
    assert_eq!(
        query.data(&frame, roots[0].0, roots[0].1),
        Ok(first.clone())
    );
    assert_eq!(query.entries.len(), entries);
    assert_eq!(
        query.data(&frame, roots[1].0, roots[1].1),
        Err(InstantiationFailure::WorkLimit)
    );
    assert!(query.entries.is_empty());
    assert!(query.pending.is_empty());
    *query.work = budget(2);
    assert_eq!(
        query.data(&frame, roots[1].0, roots[1].1),
        Err(InstantiationFailure::WorkLimit)
    );
    assert!(query.entries.is_empty());
    assert!(query.pending.is_empty());
    *query.work = budget(100_000);
    assert_eq!(query.data(&frame, roots[1].0, roots[1].1), Ok(second));
    assert_eq!(query.data(&frame, roots[0].0, roots[0].1), Ok(first));
}
