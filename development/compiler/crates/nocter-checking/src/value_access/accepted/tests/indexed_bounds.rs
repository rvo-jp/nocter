//! One checking-owned disposition, independent of later Machine constant propagation.
use super::*;
use nocter_model::IndexBoundsCheck;

#[test]
fn bounds_facts_classify_only_normally_produced_numeric_values() {
    for (source, expected) in [
        (
            "func work(values: [i32; 2]): i32 { values[1] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "const INDEX: usize = 1\nfunc work(values: [i32; 2]): i32 { values[INDEX] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [i32; 2], flag: bool): i32 { values[if flag { 1 } else { 1 }] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [void; 18446744073709551615]): void { values[18446744073709551614] }",
            IndexBoundsCheck::ProvenInBounds,
        ),
        (
            "func work(values: [i32; 2], index: usize): i32 { values[index] }",
            IndexBoundsCheck::Required,
        ),
        (
            "func work(values: [i32; 2]): i32 { values[2] }",
            IndexBoundsCheck::ProvenTrap,
        ),
        (
            "func work(values: [void; 0]): void { values[0] }",
            IndexBoundsCheck::ProvenTrap,
        ),
        (
            "func work(): void { let values = [0]\nlet _ = values[if true { return } else { 0 }] }",
            IndexBoundsCheck::Required,
        ),
    ] {
        let unit = accepted(source);
        let query = named(&unit, "work");
        let checks: Vec<_> = query
            .typed()
            .body()
            .expressions()
            .iter()
            .filter_map(|(_, value)| match value.kind {
                crate::value_access::ExpressionKind::Index { bounds, .. } => Some(bounds),
                _ => None,
            })
            .collect();
        assert_eq!(checks, [expected], "{source}");
    }
}

#[test]
fn guaranteed_failure_produces_no_value_alias_call_commit_or_unwinding_cleanup() {
    for body in [
        "let _ = values[0]",
        "read selected = values[0]",
        "edit selected = values[0]",
        "consume(input^, values[0])",
        "values[0] = replacement^",
    ] {
        let source = format!(
            "func consume(take input: error, value: error): void {{}}\nfunc work(take input: error, take replacement: error): void {{ var values: [error; 0] = []\n{body} }}"
        );
        let unit = accepted(&source);
        let query = named(&unit, "work");
        let actions: Vec<_> = query
            .graph()
            .points()
            .iter()
            .flat_map(|(_, block)| block.actions.iter())
            .collect();
        let failure = actions
            .iter()
            .find_map(|event| match event.operation {
                Operation::IndexFailure(id) => Some(id),
                _ => None,
            })
            .expect("explicit no-result failure");
        assert!(!actions.iter().any(|event| matches!(event.operation,
            Operation::Evaluate(id) if id == failure)));
        assert!(!actions.iter().any(|event| matches!(
            event.operation,
            Operation::CommitCall(_)
                | Operation::CancelCall { .. }
                | Operation::Assign { .. }
                | Operation::Bind { .. }
        ) && match event.operation {
            Operation::Bind { value, .. } => value == failure,
            _ => true,
        }));
        assert_eq!(
            actions
                .iter()
                .filter(|event| matches!(event.operation, Operation::Bind { .. }))
                .count(),
            1
        );
        let trace = super::support::steps(query);
        assert!(
            !trace.iter().any(|step| matches!(
                step,
                Step::Dispose { .. } | Step::ReplaceReferent { .. } | Step::Invoke { .. }
            )),
            "a non-unwinding trap must not dispose prior owners: {body}"
        );
    }
}

#[test]
fn a_zero_repetition_disposes_its_seed_before_the_later_safety_trap() {
    let unit = accepted(
        "func work(take input: error): void { let values = [input^; 0]\nlet _ = values[0] }",
    );
    let query = named(&unit, "work");
    let trace = super::support::steps(query);
    assert_eq!(
        trace
            .iter()
            .filter(|step| matches!(step,
                Step::Dispose { payload, .. } if payload.disposal() == Disposal::OwnedError
            ))
            .count(),
        1
    );
    let (point, block) = query
        .graph()
        .points()
        .iter()
        .find(|(_, block)| {
            block
                .actions
                .iter()
                .any(|event| matches!(event.operation, Operation::IndexFailure(_)))
        })
        .unwrap();
    let action = block.actions.len() - 1;
    assert!(matches!(
        block.actions[action].operation,
        Operation::IndexFailure(_)
    ));
    assert!(matches!(block.terminator, flow::Terminator::Unreachable));
    assert!(
        query
            .access()
            .resources()
            .at(Location { point, action })
            .all(|transition| transition.steps().is_empty())
    );
}

#[test]
fn required_failure_is_local_and_does_not_demand_a_nonexistent_element_header() {
    use nocter_constant_evaluation::state::Rule as ExecutionRule;
    for source in [
        "struct Item { value: i32 }\nconst ANSWER: Item = [Item { value: 1 }][1]",
        "struct Node { payload: [i32; COUNT] }\nconst COUNT: usize = compute()\nconst func compute(): usize { let empty: [Node; 0] = []\nlet _ = empty[0]\n0 }",
        "const func compute(): void { let empty: [error; 0] = []\nlet _ = empty[0] }\nconst ANSWER: void = compute()",
        "const func compute(): void { var empty: [error; 0] = []\nedit selected = empty[0] }\nconst ANSWER: void = compute()",
    ] {
        let (sources, tree) = input(source);
        let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("expected a local execution failure: {source}");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(ExecutionRule::IndexOutOfBounds),
            "{source}: {:?}",
            diagnostic.source()
        );
        let span = sources
            .get(tree.source())
            .unwrap()
            .text_at(diagnostic.source().primary().span().range())
            .unwrap();
        assert!(span.ends_with("[0]") || span.ends_with("[1]"), "{span}");
    }
}

#[test]
fn operand_exit_precedes_the_proven_trap_and_keeps_prior_effects() {
    use nocter_constant_evaluation::state::Value;
    let source = "const func make(edit log: i32): [i32; 0] { log += 1\n[] }\nconst func inner(edit log: i32): i32 { let _ = make(log)[if true { return 41 } else { 0 }]\n0 }\nconst func work(): i32 { var log = 0\nlet result = inner(log)\nresult + log }\nconst ANSWER: i32 = work()";
    assert_eq!(
        super::initializers::answer(source, "ANSWER"),
        Value::integer(42)
    );
    let unit = accepted(source);
    assert!(
        named(&unit, "inner")
            .graph()
            .points()
            .iter()
            .filter(|(point, _)| named(&unit, "inner").initialization().reachable(*point))
            .all(|(_, block)| block
                .actions
                .iter()
                .all(|event| !matches!(event.operation, Operation::IndexFailure(_))))
    );
}

#[test]
fn operand_failures_precede_a_later_guaranteed_bounds_failure() {
    use nocter_constant_evaluation::state::Rule as ExecutionRule;
    for source in [
        "const func make(): [i32; 0] { let _ = 1 / 0\n[] }\nconst ANSWER: i32 = make()[0]",
        "const func position(): usize { let _ = 1 / 0\n0 }\nconst ANSWER: void = [(); 0][position()]",
    ] {
        let (sources, tree) = input(source);
        let Err(SourceFailure::Checking(Failure::Evaluation { diagnostic, .. })) =
            SourceUnit::prepare(bound(&sources, &tree))
        else {
            panic!("operand failure: {source}");
        };
        assert_eq!(
            diagnostic.rule(),
            crate::value_access::evaluation::Rule::Execution(ExecutionRule::ArithmeticFailure)
        );
    }
}
