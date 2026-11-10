use crate::value_access::{flow::Query, tests::queries};
use std::sync::Arc;
fn analyze(text: &str) -> Vec<Query> {
    queries(text)
        .unwrap()
        .into_iter()
        .map(|q| Query::prepare(Arc::new(q)).unwrap())
        .collect()
}
fn pass(text: &str) {
    for query in analyze(text) {
        assert!(query.initialization().violations().is_empty(), "{text}");
        assert!(
            query.access().violations().is_empty(),
            "{text}: {:?}",
            query.access().violations()
        );
    }
}
fn reject(text: &str) {
    assert!(
        analyze(text)
            .iter()
            .any(|q| !q.access().violations().is_empty()),
        "{text}"
    );
}
#[test]
fn alias_last_use_allows_parent_updates_but_not_earlier_ones() {
    pass(
        "func f(): i32 { var value = 1\nread old = value\nlet copy = old\nvalue = 2\nreturn copy + value }",
    );
    reject("func f(): i32 { var value = 1\nread old = value\nvalue = 2\nreturn old }");
}
#[test]
fn explicit_edit_alias_activates_before_a_call_reservation_would() {
    reject(
        "func observe(x: i32): void {}\nfunc f(): i32 { var value = 1\nedit working = value\nobserve(value)\nworking = 2\nreturn value }",
    );
    pass(
        "func observe(x: i32): i32 { x }\nfunc replace(edit target: i32,x: i32): void { target = x }\nfunc f(): i32 { var value = 1\nreplace(value,observe(value))\nreturn value }",
    );
}
#[test]
fn copy_take_snapshot_is_disjoint_from_a_later_edit_of_its_source() {
    pass(
        "func use_both(take old: i32,edit current: i32): void { current = old }\nfunc f(): i32 { var value = 1\nuse_both(value,value)\nreturn value }",
    );
    reject(
        "func use_both(old: i32,edit current: i32): void {}\nfunc f(): void { var value = 1\nuse_both(value,value) }",
    );
}
#[test]
fn duplicate_noncopy_transfer_and_live_alias_transfer_are_rejected() {
    reject(
        "func consume(take a: error,take b: error): void {}\nfunc f(take input: error): void { consume(input^,input^) }",
    );
    reject(
        "func observe(x: error): void {}\nfunc f(take input: error): error { read alias = input\nlet moved = input^\nobserve(alias)\nreturn moved^ }",
    );
}
#[test]
fn unused_edit_intermediary_does_not_keep_exclusive_demand_after_read_child() {
    pass(
        "func f(): i32 { var value = 1\nedit parent = value\nread child = parent\nread sibling = value\nreturn child + sibling }",
    );
}
#[test]
fn reservation_blocks_a_later_argument_from_modifying_its_target() {
    reject(
        "func change(edit input: i32): i32 { input += 1\nreturn input }\nfunc observe(input: i32,other: i32): void {}\nfunc f(): void { var value = 1\nobserve(value,change(value)) }",
    );
}

#[test]
fn index_side_invalidation_reports_a_retained_place_rather_than_an_activated_read() {
    let source = "func work(): i32 { var values = [42, 7]\nvalues[if true { values = [1, 2]\n0 } else { 0 }] }";
    assert!(
        analyze(source)
            .iter()
            .flat_map(|q| q.access().violations())
            .any(|v| v
                .retained
                .is_some_and(|r| r.kind == super::RetentionKind::Place))
    );
}
