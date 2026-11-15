//! Logical producer structure is independent of runtime payloads and source capture admission.
use super::*;
use nocter_model::ValueApplication;
use nocter_model::{AccessCodeId, AccessExpressionId, ArenaBuilder, ValueInvocation};

fn applications() -> [FunctionWitness; 2] {
    let mut codes = ArenaBuilder::<AccessCodeId, ()>::new();
    let owner = codes.insert(()).into();
    let mut calls = ArenaBuilder::<AccessExpressionId, ()>::new();
    [0, 1].map(|_| FunctionWitness {
        origin: ValueFunctionOrigin::Application(
            ValueApplication {
                owner,
                call: calls.insert(()),
            }
            .into(),
        ),
        invocation: ValueInvocation::Read,
    })
}

#[test]
fn explicit_tags_do_not_survive_unknown_paths_or_weak_updates_as_certificates() {
    let mut store = Store::default();
    let unknown = Store::unknown();
    let absent = store.outcome(BTreeMap::new());
    let present = store.outcome(BTreeMap::from([(ValueField::Variant(true), unknown)]));
    assert_eq!(store.nodes[absent.0].tag.primary(), Some(false));
    assert_eq!(store.nodes[present.0].tag.primary(), Some(true));
    for other in [unknown, present] {
        let merged = store.join(absent, other);
        assert_eq!(store.nodes[merged.0].tag.primary(), None);
        assert_eq!(merged, store.join(other, absent));
    }
    let product = store.aggregate(BTreeMap::from([(ValueField::Tuple(0), absent)]));
    let merged = store.join(product, unknown);
    let selected = store.selected(merged, &[ValueField::Tuple(0)]);
    assert_eq!(store.nodes[selected.0].tag.primary(), None);
    let weak = store.update(unknown, &[ValueField::Array(0)], absent, false);
    let selected = store.selected(weak, &[ValueField::Array(0)]);
    assert_eq!(store.nodes[selected.0].tag.primary(), None);
    let exact = store.update(product, &[ValueField::Tuple(0)], present, false);
    assert_eq!(
        store.nodes[store.selected(exact, &[ValueField::Tuple(0)]).0]
            .tag
            .primary(),
        Some(true)
    );
    assert_eq!(
        store.nodes[store.selected(product, &[ValueField::Tuple(0)]).0]
            .tag
            .primary(),
        Some(false)
    );
    assert_eq!(
        store.nodes[store.selected(product, &[ValueField::Tuple(1)]).0]
            .tag
            .primary(),
        None
    );
}

#[test]
fn deep_shared_join_is_iterative_and_does_not_expand_the_logical_value_tree() {
    const DEPTH: usize = 4096;
    let [first, second] = applications();
    let mut store = Store::default();
    let mut left = store.single(first);
    let mut right = store.single(second);
    for _ in 0..DEPTH {
        left = store.aggregate(BTreeMap::from([
            (ValueField::Tuple(0), left),
            (ValueField::Tuple(1), left),
        ]));
        right = store.aggregate(BTreeMap::from([
            (ValueField::Tuple(0), right),
            (ValueField::Tuple(1), right),
        ]));
    }
    let joined = store.join(left, right);
    assert_eq!(store.join(right, left), joined);
    assert_eq!(store.join(joined, joined), joined);
    assert!(
        store.nodes.len() <= 4 * (DEPTH + 1),
        "work follows shared structure, not 2^DEPTH leaves"
    );
    let path = vec![ValueField::Tuple(1); DEPTH];
    assert_eq!(
        store.producers(store.selected(joined, &path)),
        Producers::from([first, second])
    );
    assert_eq!(store.applications(joined).len(), 2);
}

#[test]
fn an_array_write_retains_other_elements_without_mixing_disjoint_element_fields() {
    let [first, second] = applications();
    let mut store = Store::default();
    let first_value = store.single(first);
    let second_value = store.single(second);
    let pair = store.aggregate(BTreeMap::from([
        (ValueField::Tuple(0), first_value),
        (ValueField::Tuple(1), second_value),
    ]));
    let array = store.array(pair);
    let updated = store.update(
        array,
        &[ValueField::Array(u64::MAX), ValueField::Tuple(0)],
        second_value,
        false,
    );
    assert_eq!(
        store.producers(store.selected(updated, &[ValueField::Array(0), ValueField::Tuple(0)])),
        Producers::from([first, second])
    );
    assert_eq!(
        store.producers(store.selected(updated, &[ValueField::Array(17), ValueField::Tuple(1)])),
        Producers::from([second])
    );
    assert_eq!(
        store.producers(store.selected(array, &[ValueField::Array(0), ValueField::Tuple(0)])),
        Producers::from([first]),
        "a new fact does not mutate an earlier snapshot"
    );
}
