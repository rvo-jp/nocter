//! Independently supplied header data must agree with ordinary required source execution.
use super::*;
use nocter_model::{ConstantValue, FrozenData, FrozenNode};

#[test]
fn mismatched_supplied_header_counts_cannot_certify_a_complete_unit() {
    let (sources, tree) = input("func run(): [i32; 1] { [41, 42] }");
    let mut headers = declarations::Preparation::prepare(bound(&sources, &tree)).unwrap();
    let function = headers.declaration_ids().next().unwrap();
    let declarations::HeaderProgress::RequireValues(counts) =
        headers.advance_with_values(function, |_| None).unwrap()
    else {
        panic!("header count")
    };
    assert_eq!(counts.len(), 1);
    headers.advance(counts[0]).unwrap();
    let substituted = FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Integer(2))]).unwrap();
    assert_eq!(
        headers
            .advance_with_values(function, |_| Some(&substituted))
            .unwrap(),
        declarations::HeaderProgress::Ready
    );
    let context = headers.current();
    assert!(context.program().headers_complete());
    assert!(
        accepted::Unit::prepare(&context).is_err(),
        "source count 1 must not be certified through metadata normalized using 2"
    );
}

fn supplied_headers<'source>(
    sources: &'source SourceMap,
    tree: &'source SyntaxTree,
    value: u64,
) -> declarations::Preparation<'source> {
    let mut headers = declarations::Preparation::prepare(bound(sources, tree)).unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut pending = headers
        .declaration_ids()
        .chain(headers.code_ids().map(Into::into))
        .filter(|id| seen.insert(*id))
        .collect::<Vec<_>>();
    let supplied = FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Integer(
        value.into(),
    ))])
    .unwrap();
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|id| {
            !matches!(
                headers
                    .advance_with_values(*id, |_| Some(&supplied))
                    .unwrap(),
                declarations::HeaderProgress::Ready
            )
        });
        assert!(pending.len() < before);
    }
    headers
}

#[test]
fn actual_counts_reconcile_data_and_callback_contracts_without_rejecting_matching_results() {
    for template in [
        "func run(): [i32; COUNT] { [41, 42] }",
        "func run(values: [i32; COUNT]): void {}",
        "func run(action: func([i32; COUNT]): void): void {}",
        "func run(values: ([i32; COUNT], bool)): void {}",
        "struct Data { values: [i32; COUNT] }\nfunc run(): Data { Data { values: [41, 42] } }",
    ] {
        for matches in [false, true] {
            let text = template.replace("COUNT", if matches { "1 + 1" } else { "1" });
            let (sources, tree) = input(&text);
            let headers = supplied_headers(&sources, &tree, 2);
            let context = headers.current();
            let run = context
                .program()
                .resolve(context.program().symbols().get("run").unwrap())
                .unwrap();
            Query::prepare(&context, run).unwrap();
            let result = accepted::Unit::prepare(&context);
            if matches {
                assert!(result.is_ok(), "{text}: {result:?}");
            } else {
                assert!(
                    matches!(result, Err(accepted::Failure::Integrity(_))),
                    "{text}: {result:?}"
                );
            }
        }
    }
}
