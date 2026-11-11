use super::*;
use crate::value_access::flow::{Operation, Query};
use crate::value_access::tests::{named, queries};

fn function(text: &str) -> Query {
    let typed = queries(text).unwrap();
    Query::prepare(Arc::new(named(&typed, "f").clone())).unwrap()
}
fn returns(query: &Query) -> Vec<(AccessExpressionId, NumericFacts)> {
    query
        .graph()
        .points()
        .iter()
        .flat_map(|(point, body)| {
            body.actions
                .iter()
                .enumerate()
                .filter_map(move |(action, item)| {
                    if let Operation::StageReturn(value) = item.operation {
                        Some((value, query.numeric_at(Location { point, action }).unwrap()))
                    } else {
                        None
                    }
                })
        })
        .collect()
}

#[test]
fn checked_offset_chain_survives_local_reassignment_and_compound_updates() {
    let query = function(
        "func f(take input: usize): usize { var next = input\nlet previous = next\nnext += 1\nnext = next + 2\nreturn next - previous }",
    );
    let result = returns(&query);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1.constant(result[0].0), Some(3));
}

#[test]
fn calls_invalidate_mutable_local_facts_but_not_prepared_operands() {
    let query = function(
        "func change(edit value: usize): usize { value = 9\nreturn 1 }\nfunc f(): usize { var input: usize = 4\nreturn input + change(input) }",
    );
    let result = returns(&query);
    // The left operand is copied before change executes; its current local is now unknown.
    let call = query
        .typed()
        .body()
        .expressions()
        .iter()
        .find_map(|(id, expression)| {
            matches!(expression.kind, ExpressionKind::Call { .. }).then_some(id)
        })
        .unwrap();
    assert_eq!(result[0].1.constant(result[0].0), None);
    assert_eq!(result[0].1.difference(result[0].0, call), Some(4));
    let query = function(
        "func change(edit value: usize): void { value = 9 }\nfunc f(): usize { var input: usize = 4\nchange(input)\nreturn input }",
    );
    let result = returns(&query);
    assert_eq!(result[0].1.constant(result[0].0), None);
}

#[test]
fn equalities_refine_only_the_matching_branch_and_joins_keep_common_differences() {
    let query = function(
        "func f(take a: usize, take b: usize): usize { if a == b { return a - b }\nreturn a - b }",
    );
    let mut answers: Vec<_> = returns(&query)
        .iter()
        .map(|(id, facts)| facts.constant(*id))
        .collect();
    answers.sort();
    assert_eq!(answers, vec![None, Some(0)]);
    let query = function(
        "func f(flag: bool): usize { var a: usize = 3\nvar b: usize = 5\nif flag { a = 8\nb = 10 }\nreturn b - a }",
    );
    let result = returns(&query);
    assert_eq!(result[0].1.constant(result[0].0), Some(2));
}

#[test]
fn loop_reexecution_does_not_reuse_a_previous_expression_value() {
    let query = function(
        "func f(again: bool): usize { var index: usize = 0\nwhile again { index += 1 }\nreturn index }",
    );
    let result = returns(&query);
    assert_eq!(result[0].1.constant(result[0].0), None);
    let query = function(
        "func f(again: bool): usize { var index: usize = 0\nwhile again { let old = index\nindex += 1\nif again { return index - old } }\nreturn 1 }",
    );
    for (id, facts) in returns(&query) {
        assert_eq!(facts.constant(id), Some(1));
    }
}

#[test]
fn unknown_operations_and_alias_writes_do_not_create_offset_proofs() {
    for body in ["input * 2", "input / 2", "input % 2"] {
        let query = function(&format!("func f(take input: usize): usize {{ {body} }}"));
        let result = returns(&query);
        assert_eq!(result[0].1.constant(result[0].0), None, "{body}");
    }
    let query = function(
        "func f(): usize { var index: usize = 4\nedit alias = index\nalias = 9\nreturn index }",
    );
    let result = returns(&query);
    assert_eq!(result[0].1.constant(result[0].0), None);
}

#[test]
fn impossible_checked_results_and_unreached_locations_furnish_no_proof() {
    let query = function(
        "func f(): usize { var index: usize = 18446744073709551615\nindex += 1\nreturn index }",
    );
    let result = returns(&query);
    assert_eq!(result[0].1.constant(result[0].0), None);
    let point = query.graph().points().get(query.graph().entry()).unwrap();
    assert!(
        query
            .numeric_at(Location {
                point: query.graph().entry(),
                action: point.actions.len()
            })
            .is_none()
    );
}
