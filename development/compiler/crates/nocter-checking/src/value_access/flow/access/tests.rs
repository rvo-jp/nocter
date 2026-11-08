use super::*;
use crate::value_access::{
    Failure, Query as TypedQuery,
    flow::{
        Query,
        tests::{named, queries},
    },
    tests::{bound, field, input, string},
};
use nocter_declaration_lowering::access_declarations as declarations;
use nocter_json::Value as Json;
use std::{fs, path::PathBuf, sync::Arc};

fn passing(source: &str) -> Vec<Query> {
    let all = queries(source);
    for query in &all {
        assert_eq!(
            query.access().status(),
            Status::Analyzed,
            "{source}: {:?}",
            query.initialization().violations()
        );
        assert!(
            query.access().violations().is_empty(),
            "{source}: {:?}",
            query.access().violations()
        );
    }
    all
}
fn rejected(source: &str, rule: AccessRule) {
    let all = queries(source);
    assert!(
        all.iter().any(|query| query
            .access()
            .violations()
            .iter()
            .any(|violation| violation.rule == rule)),
        "{source}: {:?}",
        all.iter()
            .map(|query| query.access().violations())
            .collect::<Vec<_>>()
    );
}

#[test]
fn corpus_retains_all_nineteen_positive_units_and_rejects_the_four_remaining_access_cases() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/v2-access");
    let manifest =
        nocter_json::parse(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let Json::Array(cases) = field(&manifest, "cases") else {
        panic!("cases");
    };
    let mut counts = [0; 3];
    for case in cases {
        if matches!(
            string(field(case, "expectation")),
            "syntax-reject" | "declaration-reject"
        ) || string(field(case, "body_typing")) == "reject"
        {
            continue;
        }
        let text = fs::read_to_string(root.join(string(field(case, "file")))).unwrap();
        let (sources, tree) = input(&text);
        let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
        let current = header.bind(bound(&sources, &tree)).unwrap();
        let mut failed = false;
        let mut blocked = false;
        let mut primary = false;
        for (id, _) in current.program().callables().iter() {
            let query =
                Query::prepare(Arc::new(TypedQuery::prepare(&current, id).unwrap())).unwrap();
            failed |= !query.access().violations().is_empty();
            blocked |= query.access().status() == Status::BlockedByInitialization;
            for diagnostic in query.bind(&current).unwrap().access_diagnostics().unwrap() {
                let required = string(field(case, "primary_text"));
                let start = text.find(required).unwrap();
                let range = diagnostic.source().primary().span().range();
                primary |= usize::try_from(range.start().get()).unwrap() >= start
                    && usize::try_from(range.end().get()).unwrap() <= start + required.len();
                assert!(!diagnostic.source().notes().is_empty());
            }
        }
        match string(field(case, "referent_access")) {
            "blocked" => {
                assert!(blocked && !failed);
                counts[0] += 1;
            }
            "reject" => {
                assert!(failed && primary, "{}", string(field(case, "id")));
                counts[1] += 1;
            }
            "pass" => {
                assert!(!failed && !blocked, "{}", string(field(case, "id")));
                counts[2] += 1;
            }
            _ => panic!("expectation"),
        }
    }
    assert_eq!(counts, [7, 4, 19]);
}

#[test]
fn last_use_releases_exclusive_access_without_waiting_for_scope_end() {
    passing(
        "func run(value: ^i32): ^i32 {\n let edit = &value\n edit += 1\n value += 1\n return ^value\n}\n",
    );
    rejected(
        "func run(value: ^i32): void {\n let edit = &value\n value += 1\n edit += 1\n}\n",
        AccessRule::ConflictingUpdate,
    );
}
#[test]
fn dead_exclusive_intermediary_downgrades_to_its_live_read_child() {
    passing(
        "func run(value: ^i32): i32 {\n let edit = &value\n let inspect = edit\n let other = value\n return inspect + other\n}\n",
    );
    rejected(
        "func run(value: ^i32): i32 {\n let edit = &value\n let inspect = edit\n value += 1\n return inspect\n}\n",
        AccessRule::ConflictingUpdate,
    );
    rejected(
        "func run(value: ^i32): void {\n let edit = &value\n let child: &i32 = edit\n let inspect = value\n child += 1\n}\n",
        AccessRule::ConflictingRead,
    );
}
#[test]
fn a_live_parent_update_cannot_bypass_a_read_child() {
    rejected(
        "func run(value: ^i32): i32 {\n let edit = &value\n let inspect = edit\n edit += 1\n return inspect\n}\n",
        AccessRule::ConflictingUpdate,
    );
}
#[test]
fn reservations_allow_children_that_end_before_activation() {
    passing(
        "func read(value: i32): i32 { value }\nfunc add(value: &i32, amount: i32): void { value += amount }\nfunc run(value: ^i32): ^i32 {\n let edit = &value\n add(value, read(edit))\n return ^value\n}\n",
    );
    rejected(
        "func read(value: i32): i32 { value }\nfunc add(value: &i32, amount: i32): void { value += amount }\nfunc run(value: ^i32): void {\n let edit = &value\n add(edit, read(value))\n}\n",
        AccessRule::ConflictingRead,
    );
}
#[test]
fn reserved_values_cannot_be_replaced_even_when_the_slot_stays_initialized() {
    rejected(
        "func take(value: ^i32, amount: i32): void {}\nfunc run(value: ^i32): void {\n take(value, if true { value = 2\n 0 } else { 0 })\n}\n",
        AccessRule::ReservedModification,
    );
}
#[test]
fn complete_bundle_rejects_duplicate_ownership_and_update_but_accepts_shared_reads() {
    passing(
        "func read(left: i32, right: i32): void {}\nfunc run(value: ^i32): void { read(value, value) }\n",
    );
    rejected(
        "func take(left: ^i32, right: ^i32): void {}\nfunc run(value: ^i32): void { take(value, value) }\n",
        AccessRule::ConflictingArguments,
    );
    rejected(
        "func edit(left: &i32, right: &i32): void {}\nfunc run(value: ^i32): void { edit(value, value) }\n",
        AccessRule::ConflictingArguments,
    );
}
#[test]
fn private_result_candidates_are_alternative_worlds() {
    let all = passing(
        "func choose(left: &i32, right: &i32, flag: bool): &i32 {\n if flag { return left } else { return right }\n}\nfunc run(left: ^i32, right: ^i32, flag: bool): void {\n let edit = choose(left, right, flag)\n edit += 1\n}\n",
    );
    assert_eq!(named(&all, "choose").access().returns().len(), 2);
}
#[test]
fn returned_access_must_refer_to_a_surviving_eligible_input() {
    passing(
        "func read(value: error): error { value }\nfunc edit(value: &i32): &i32 { value }\nfunc copy(value: ^i32): i32 { value }\nfunc forward(value: ^error): ^error { ^value }\n",
    );
    rejected(
        "func run(value: ^error): error { value }\n",
        AccessRule::InvalidReturnRoot,
    );
    rejected(
        "func run(input: &i32): &i32 {\n let local = 0\n let child = &local\n return child\n}\n",
        AccessRule::InvalidReturnRoot,
    );
}
#[test]
fn loop_liveness_preserves_parent_restrictions_on_backedges() {
    passing(
        "func run(value: ^i32, flag: bool): ^i32 {\n while flag {\n let edit = &value\n edit += 1\n value += 1\n }\n return ^value\n}\n",
    );
    rejected(
        "func run(value: ^i32, flag: bool): void {\n let edit = &value\n while flag {\n value += 1\n edit += 1\n }\n}\n",
        AccessRule::ConflictingUpdate,
    );
}
#[test]
fn source_diagnostics_rebind_the_root_retained_access_and_later_use() {
    let text = "func run(value: ^i32): void {\n let edit = &value\n value += 1\n edit += 1\n}\n";
    let (sources, tree) = input(text);
    let header = declarations::Query::prepare(bound(&sources, &tree)).unwrap();
    let current = header.bind(bound(&sources, &tree)).unwrap();
    let id = current.program().callables().iter().next().unwrap().0;
    let query = Query::prepare(Arc::new(TypedQuery::prepare(&current, id).unwrap())).unwrap();
    let (later_sources, later_tree) = input(&format!("\n{text}"));
    let later = header.bind(bound(&later_sources, &later_tree)).unwrap();
    let context = query.bind(&later).unwrap();
    let diagnostics = context.access_diagnostics().unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].source().notes().len(), 3);
    assert!(
        diagnostics[0]
            .source()
            .notes()
            .iter()
            .all(|note| note.origin().source() == later_tree.source())
    );
    assert_eq!(
        diagnostics[0].source().primary().source(),
        later_tree.source()
    );
    let (changed_sources, changed_tree) = input(&text.replace("value += 1", "value += 2"));
    let changed = header.bind(bound(&changed_sources, &changed_tree)).unwrap();
    assert!(matches!(query.bind(&changed), Err(Failure::ChangedBody)));
}

#[test]
fn pending_read_does_not_retain_unused_update_authority() {
    passing(
        "func read(left: i32, right: i32): i32 { left + right }\nfunc run(value: ^i32): i32 {\n let edit = &value\n return read(edit, value)\n}\n",
    );
    rejected(
        "func read(left: i32, right: i32): i32 { left + right }\nfunc run(value: ^i32): i32 {\n let edit = &value\n let answer = read(edit, value)\n edit += 1\n return answer\n}\n",
        AccessRule::ConflictingRead,
    );
}
#[test]
fn block_cleanup_protects_forwarded_access_but_allows_an_explicit_copy() {
    rejected(
        "func run(flag: bool): i32 {\n let inspect = if flag { let local = 1\n local } else { let other = 2\n other }\n return inspect\n}\n",
        AccessRule::ConflictingTransfer,
    );
    passing(
        "func run(flag: bool): i32 {\n let answer: ^i32 = if flag { let local = 1\n local } else { let other = 2\n other }\n return answer\n}\n",
    );
}
#[test]
fn zero_sized_roots_keep_logical_identity() {
    passing(
        "func run(left: ^void, right: ^void): bool {\n let edit = &left\n let inspect = right\n let same = inspect == ()\n edit = ()\n return same\n}\n",
    );
    rejected(
        "func run(value: ^void): bool {\n let edit = &value\n let inspect = value\n edit = ()\n return inspect == ()\n}\n",
        AccessRule::ConflictingRead,
    );
}

#[test]
fn remaining_read_uses_do_not_extend_exclusivity_after_the_last_update() {
    passing(
        "func run(value: ^i32): i32 {\n let edit = &value\n let inspect = edit\n let other = value\n return inspect + other + edit\n}\n",
    );
    rejected(
        "func run(value: ^i32): i32 {\n let edit = &value\n let inspect = edit\n let other = value\n edit += 1\n return inspect + other\n}\n",
        AccessRule::ConflictingRead,
    );
    rejected(
        "func run(value: ^i32): i32 {\n let inspect = value\n let unused = &value\n return inspect\n}\n",
        AccessRule::ConflictingUpdate,
    );
}

#[test]
fn releasing_a_computed_access_during_cancellation_does_not_prolong_its_root_use() {
    passing(
        "func take(value: &i32, other: i32): void {}\nfunc run(value: ^i32, flag: bool): ^i32 {\n take(if flag { &value } else { &value }, if flag { return ^value } else { 0 })\n return ^value\n}\n",
    );
}

#[test]
fn cancelled_update_reservation_keeps_identity_protection_without_unneeded_exclusivity() {
    passing(
        "func take(value: &i32, other: i32): void {}\nfunc run(value: ^i32, flag: bool): i32 {\n let edit = &value\n take(edit, if flag { return value } else { 0 })\n return value\n}\n",
    );
    rejected(
        "func take(value: &i32, other: i32): void {}\nfunc run(value: ^i32, flag: bool): ^i32 {\n let edit = &value\n take(edit, if flag { return ^value } else { 0 })\n return ^value\n}\n",
        AccessRule::ReservedModification,
    );
}
