use super::*;
use crate::value_access::flow::{Operation, Query};
use crate::value_access::{
    evaluation, results,
    tests::{bound, input},
    types,
};
use nocter_declaration_lowering::access_declarations as declarations;

fn function(text: &str) -> Query {
    function_named(text, "f")
}
fn function_named(text: &str, name: &str) -> Query {
    // Drive ordinary staged typing/count supply and result closure, stopping before whole-unit
    // safety admission. Several cases intentionally violate availability: numerical proof alone
    // must neither accept those programs nor pretend to execute their cleanup.
    let (sources, tree) = input(text);
    let syntax = bound(&sources, &tree);
    let headers = declarations::Preparation::prepare(syntax).unwrap();
    let (header, output) = evaluation::prepare_source(
        headers,
        nocter_constant_evaluation::state::Limits::default(),
    )
    .unwrap();
    let context = header.bind(syntax).unwrap();
    let mut queries = output.queries;
    types::canonicalize(&context, &mut queries).unwrap();
    let mut controls = queries
        .into_iter()
        .map(|(id, query)| (id, super::super::Control::prepare(Arc::new(query)).unwrap()))
        .collect();
    results::close(&context, &mut controls).unwrap();
    let writes = super::super::call_writes::Catalog::prepare(controls.values()).unwrap();
    controls
        .into_values()
        .find(|control| {
            let typed = &control.typed;
            matches!(typed.code(), nocter_model::ValueCode::Declaration(_))
                && typed
                    .program()
                    .codes()
                    .get(typed.source_owner())
                    .unwrap()
                    .name()
                    .and_then(|name| typed.program().symbols().spelling(name))
                    == Some(name)
        })
        .unwrap()
        .finish(&writes)
        .unwrap()
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
    for (point, body) in query.graph().points().iter() {
        for (action, operation) in body.actions.iter().enumerate() {
            if matches!(operation.operation, Operation::StageReturn(_)) {
                assert!(query.numeric_at(Location { point, action }).is_none());
            }
        }
    }
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

fn transfer_facts(query: &Query) -> Vec<(AccessExpressionId, NumericFacts)> {
    let mut result = Vec::new();
    for (point, body) in query.graph().points().iter() {
        for (action, operation) in body.actions.iter().enumerate() {
            let Some(facts) = query.numeric_at(Location { point, action }) else {
                continue;
            };
            for source in history::transfers(query.typed(), &operation.operation).unwrap() {
                if let Referent::Evaluated(selection) = source
                    && super::super::resources::RetainedIndex::for_selection(
                        query.typed(),
                        selection,
                    )
                    .is_some()
                {
                    result.push((selection, facts.clone()));
                }
            }
        }
    }
    result
}
fn prefix_transfers(query: &Query) -> Vec<bool> {
    transfer_facts(query)
        .iter()
        .map(|(selection, facts)| facts.extends_prefix(*selection))
        .collect()
}
fn frontiers(query: &Query) -> Vec<Option<ArrayFrontier>> {
    transfer_facts(query)
        .iter()
        .map(|(selection, facts)| facts.prefix_frontier(*selection))
        .collect()
}

#[test]
fn prefix_induction_reaches_a_finite_fixed_point_independent_of_array_count() {
    let mut sizes = Vec::new();
    for count in [2, 100_000, u64::MAX] {
        let query = function(&format!(
            "func f(take items: [void; {count}]): void {{ var index: usize = 0\nwhile index < {count} {{ let selected = items[index]^\nindex += 1 }} }}"
        ));
        assert_eq!(prefix_transfers(&query), vec![true]);
        let frontier = frontiers(&query)[0].unwrap();
        assert_eq!(frontier.owner(), query.typed().code());
        assert_eq!(frontier.bound(), count);
        assert_eq!(
            frontier.array(query.typed()).unwrap().holder(),
            super::super::resources::Holder::Local(
                query.typed().body().inputs().explicit()[0].local
            )
        );
        sizes.push((
            query.numeric.cells.count,
            query.numeric.entries.len(),
            query.numeric.cells.histories.len(),
            query.numeric.progressions.prefixes.len(),
        ));
    }
    assert!(sizes.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn missing_first_step_repeated_index_and_stride_gaps_have_no_prefix_proof() {
    for (start, step) in [(1, 1), (0, 0), (0, 2)] {
        let query = function(&format!(
            "func f(take items: [void; 8], again: bool): void {{ var index: usize = {start}\nwhile again {{ let selected = items[index]^\nindex += {step} }} }}"
        ));
        assert_eq!(
            prefix_transfers(&query),
            vec![false],
            "start={start}, step={step}"
        );
        assert!(frontiers(&query).iter().all(Option::is_none));
    }
}

#[test]
fn replacement_starts_a_new_epoch_but_partial_restoration_invalidates_the_prefix() {
    let query = function(
        "func f(): void { var items = [(); 3]\nvar index: usize = 0\nlet first = items[index]^\nitems[0] = ()\nindex = 1\nlet next = items[index]^\nitems = [(); 3]\nindex = 0\nlet replaced = items[index]^ }",
    );
    assert_eq!(prefix_transfers(&query), vec![true, false, true]);
}

#[test]
fn call_bundle_uses_transfer_order_and_consumes_a_closed_callee_write_summary() {
    let query = function(
        "func consume(take a: void, take b: void): void {}\nfunc f(): void { let items = [(); 3]\nvar index: usize = 0\nconsume(items[index]^, items[index + 1]^)\nindex = 2\nlet next = items[index]^ }",
    );
    assert_eq!(prefix_transfers(&query), vec![true, true, true]);
    let query = function(
        "func consume(take a: void, edit index: usize): void { index += 1 }\nfunc f(): void { let items = [(); 3]\nvar index: usize = 0\nconsume(items[index]^, index)\nlet next = items[index]^ }",
    );
    assert_eq!(prefix_transfers(&query), vec![true, false]);
}

#[test]
fn abrupt_argument_cancellation_never_commits_the_reserved_prefix() {
    let query = function(
        "func consume(take a: void, b: usize): void {}\nfunc f(): void { let items = [(); 3]\nvar index: usize = 0\nconsume(items[index]^, if true { return () } else { 0 }) }",
    );
    assert!(prefix_transfers(&query).is_empty());
    assert!(query.graph().points().iter().any(|(point, body)| {
        body.actions.iter().enumerate().any(|(action, item)| {
            matches!(item.operation, Operation::CancelCall { prepared: 1, .. })
                && query.numeric_at(Location { point, action }).is_some()
        })
    }));
}

#[test]
fn disjoint_containing_arrays_preserve_their_own_prefix_histories() {
    let query = function(
        "struct Pair { left: [void; 3]\nright: [void; 3] }\nfunc f(): void { var pair = Pair { left: [(); 3], right: [(); 3] }\nvar index: usize = 0\nlet left = pair.left[index]^\nlet right = pair.right[index]^\npair.right[0] = ()\nindex += 1\nlet next = pair.left[index]^ }",
    );
    assert_eq!(prefix_transfers(&query), vec![true, true, true]);
}

#[test]
fn saved_selection_and_early_exit_do_not_depend_on_the_counter_update_position() {
    for body in [
        "let selected = items[index]^\nif stop { break }\nindex += 1",
        "let position = index\nindex += 1\nlet selected = items[position]^\nif stop { break }",
    ] {
        let query = function(&format!(
            "func f(take items: [void; 8], stop: bool): void {{ var index: usize = 0\nwhile index < 8 {{ {body} }} }}"
        ));
        assert_eq!(prefix_transfers(&query), vec![true], "{body}");
    }
}

#[test]
fn a_join_with_partial_restoration_cannot_hide_the_unproved_history() {
    for replacement in ["items[0] = ()", "items = [(); 3]"] {
        let query = function(&format!(
            "func f(restore: bool): void {{ var items = [(); 3]\nvar index: usize = 0\nlet first = items[index]^\nif restore {{ {replacement} }}\nindex = 1\nlet next = items[index]^ }}"
        ));
        assert_eq!(prefix_transfers(&query), vec![true, false], "{replacement}");
    }
}

#[test]
fn builtin_zero_equality_can_establish_the_first_prefix_index() {
    for body in [
        "if index == 0 { let selected = items[index]^ }",
        "if index != 0 {} else { let selected = items[index]^ }",
    ] {
        let query = function(&format!(
            "func f(take items: [void; 2], take index: usize): void {{ {body} }}"
        ));
        assert_eq!(prefix_transfers(&query), vec![true]);
    }
}

#[test]
fn straight_line_restore_and_resetting_loops_do_not_request_a_prefix_lifecycle() {
    for source in [
        "func f(): void { var items = [(); 3]\nlet selected = items[0]^\nitems[0] = () }",
        "func f(again: bool): void { while again { var items = [(); 3]\nlet first = items[0]^\nlet second = items[1]^\nitems[0] = ()\nitems[1] = () } }",
        "func f(again: bool): void { var items = [(); 3]\nwhile again { let first = items[0]^\nlet second = items[1]^\nitems = [(); 3] } }",
    ] {
        let query = function(source);
        assert!(frontiers(&query).iter().all(Option::is_none), "{source}");
        let (sources, tree) = input(source);
        // Preserve whole-unit acceptance of these already supported point/restoration cases.
        crate::value_access::accepted::SourceUnit::prepare(bound(&sources, &tree)).unwrap();
    }
}

#[test]
fn an_initial_seed_and_its_repeated_recipe_share_one_array_frontier() {
    let query = function(
        "func f(take items: [void; 3]): void { let first = items[0]^\nvar index: usize = 1\nwhile index < 3 { let next = items[index]^\nindex += 1 } }",
    );
    let bindings = frontiers(&query);
    assert_eq!(bindings.len(), 2);
    assert!(bindings[0].is_some());
    assert_eq!(bindings[0], bindings[1]);
}

#[test]
fn repetition_in_one_parent_does_not_enroll_its_disjoint_sibling() {
    let query = function(
        "struct Pair { left: [void; 3]\nright: [void; 3] }\nfunc f(take pair: Pair): void { let right = pair.right[0]^\nvar index: usize = 0\nwhile index < 3 { let left = pair.left[index]^\nindex += 1 } }",
    );
    let bindings = frontiers(&query);
    assert_eq!(bindings.len(), 2);
    assert!(bindings[0].is_none());
    assert!(bindings[1].is_some());
}

#[test]
fn exact_comparison_truth_prunes_only_impossible_numeric_continuations() {
    for (operation, right, expected) in [
        ("==", 4, true),
        ("!=", 4, false),
        ("<", 5, true),
        ("<=", 4, true),
        (">", 3, true),
        (">=", 5, false),
    ] {
        let query = function(&format!(
            "func f(): usize {{ var value: usize = 4\nif value {operation} {right} {{ return 1 }} else {{ return 0 }} }}"
        ));
        let reached: Vec<_> = query
            .graph()
            .points()
            .iter()
            .flat_map(|(point, body)| {
                body.actions
                    .iter()
                    .enumerate()
                    .filter_map(move |(action, item)| {
                        if let Operation::StageReturn(value) = item.operation {
                            Some((Location { point, action }, value))
                        } else {
                            None
                        }
                    })
            })
            .filter_map(|(at, value)| query.numeric_at(at).and_then(|facts| facts.constant(value)))
            .collect();
        assert_eq!(reached, vec![u64::from(expected)], "{operation}");
    }
}

#[test]
fn conditional_replacement_forgets_only_the_replaced_epoch() {
    let query = function(
        "func f(again: bool, reset: bool): void { var items = [(); 3]\nvar index: usize = 0\nwhile again { let selected = items[index]^\nif reset { items = [(); 3]\nindex = 0 } else { index += 1 } } }",
    );
    assert_eq!(prefix_transfers(&query), vec![true]);
    assert!(frontiers(&query)[0].is_some());
}

#[test]
fn an_array_frontier_cannot_be_projected_in_another_body_namespace() {
    let source = "func f(take items: [void; 3]): void { var index: usize = 0\nwhile index < 3 { let selected = items[index]^\nindex += 1 } }\nfunc g(take items: [void; 3]): void { read selected = items[0] }";
    let owner = function(source);
    let other = function_named(source, "g");
    let frontier = frontiers(&owner)[0].unwrap();
    assert!(frontier.array(owner.typed()).is_some());
    assert!(frontier.array(other.typed()).is_none());
}
