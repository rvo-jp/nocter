use super::*;
use crate::value_access::tests::{named, queries};
use std::sync::Arc;
fn reports(text: &str) -> Vec<Query> {
    queries(text)
        .unwrap()
        .into_iter()
        .map(|typed| Query::prepare(Arc::new(typed)).unwrap())
        .collect()
}
fn function<'a>(all: &'a [Query], name: &str) -> &'a Query {
    all.iter()
        .find(|q| {
            let t = q.typed();
            t.program().callables().get(t.callable()).unwrap().name()
                == t.program().symbols().get(name).unwrap()
        })
        .unwrap()
}
#[test]
fn copied_source_is_initialized_but_dropped_or_moved_owners_are_not() {
    let all = reports(
        "func f(): i32 { var value = 1\nlet copy = value\nvalue = 2\nreturn value + copy }",
    );
    assert!(function(&all, "f").initialization().violations().is_empty());
    for text in [
        "func f(): i32 { let value = 1\ndrop value\nreturn value }",
        "func g(value: error): void {}\nfunc f(take value: error): error { let moved = value\ng(value)\nreturn moved }",
    ] {
        assert!(
            reports(text)
                .iter()
                .any(|q| !q.initialization().violations().is_empty()),
            "{text}"
        );
    }
}
#[test]
fn every_continuing_backedge_needs_initialized_storage() {
    let bad = reports(
        "func f(take input: error,again: bool): void { var value = input\nwhile again { drop value } }",
    );
    assert!(!function(&bad, "f").initialization().violations().is_empty());
    let good = reports(
        "func f(take input: error,again: bool): error { var value = input\nwhile again { let moved = value\nvalue = moved }\nreturn value }",
    );
    assert!(
        function(&good, "f")
            .initialization()
            .violations()
            .is_empty()
    );
}
#[test]
fn rhs_first_replacement_restores_a_moved_slot_without_reading_old_storage() {
    let all = reports(
        "func identity(take value: error): error { value }\nfunc f(take input: error): error { var value = input\nvalue = identity(value)\nreturn value }",
    );
    assert!(function(&all, "f").initialization().violations().is_empty());
    assert!(function(&all, "f").access().violations().is_empty());
}
#[test]
fn boolean_literal_edges_do_not_invent_reachable_initialization_uses() {
    let all = reports("func f(): i32 { var value = 1\nif false { drop value }\nreturn value }");
    assert!(function(&all, "f").initialization().violations().is_empty());
    let all = queries("func f(): i32 { while true {} }").unwrap();
    assert!(
        Query::prepare(Arc::new(named(&all, "f").clone()))
            .unwrap()
            .initialization()
            .violations()
            .is_empty()
    );
}
