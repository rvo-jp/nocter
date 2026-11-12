//! Enumerate concrete possibilities independently of the symbolic availability query.
use super::super::InitializationRequirement as Requirement;
use super::*;
use crate::value_access::{AccessPath, IndexFact, paths};
use nocter_model::{ValueField, ValueProjection};

fn expand(path: &AccessPath) -> Vec<FieldPath> {
    let mut paths = vec![Vec::new()];
    for field in path.fields() {
        let choices = match *field {
            ValueProjection::Array(IndexFact::Known(index)) => vec![ValueField::Array(index)],
            ValueProjection::Array(IndexFact::Unknown) => {
                vec![ValueField::Array(0), ValueField::Array(1)]
            }
            ValueProjection::Tuple(index) => vec![ValueField::Tuple(index)],
            ValueProjection::Variant(primary) => vec![ValueField::Variant(primary)],
            ValueProjection::Record(_) => unreachable!(),
        };
        paths = paths
            .into_iter()
            .flat_map(|prefix| {
                choices.iter().map(move |&choice| {
                    let mut next = prefix.clone();
                    next.push(choice);
                    next
                })
            })
            .collect();
    }
    paths.into_iter().map(FieldPath::new).collect()
}
fn queries() -> BTreeSet<AccessPath> {
    let mut queries = BTreeSet::from([AccessPath::default()]);
    for a in [IndexFact::Known(0), IndexFact::Known(1), IndexFact::Unknown] {
        let outer = AccessPath::new([ValueProjection::Array(a)]);
        queries.insert(outer.clone());
        for b in [IndexFact::Known(0), IndexFact::Known(1), IndexFact::Unknown] {
            let inner = outer.project(ValueProjection::Array(b));
            queries.insert(inner.clone());
            for field in 0..2 {
                queries.insert(inner.project(ValueProjection::Tuple(field)));
            }
        }
    }
    queries
}
fn check(storage: &Storage, query: &AccessPath) {
    for requirement in [Requirement::Value, Requirement::Container] {
        let expected = expand(query).iter().all(|path| {
            storage.root.state == I::Initialized
                && storage.selections.iter().all(|(hole, slot)| {
                    expand(hole).iter().all(|hole| {
                        let overlaps = if requirement == Requirement::Value {
                            hole.is_prefix_of(path) || path.is_prefix_of(hole)
                        } else {
                            hole.is_prefix_of(path)
                        };
                        !overlaps || slot.state == I::Initialized
                    })
                })
        });
        assert_eq!(
            storage.selection(query, requirement).state == I::Initialized,
            expected,
            "{storage:?}, {query:?}, {requirement:?}"
        );
    }
}
#[test]
fn symbolic_selection_never_reads_a_possible_hole_and_keeps_disjoint_later_fields() {
    let queries = queries();
    let leaves: Vec<_> = queries
        .iter()
        .filter(|q| q.fields().len() == 3)
        .filter_map(paths::concrete)
        .collect();
    assert_eq!(leaves.len(), 8);
    for holes in 0..256 {
        let mut storage = Storage::new(I::Initialized);
        for (index, leaf) in leaves.iter().enumerate() {
            if holes & (1 << index) != 0 {
                storage.set(leaf, I::Uninitialized, None);
            }
        }
        for query in &queries {
            check(&storage, query);
        }
    }
    for hole in queries.iter().filter_map(paths::concrete) {
        for state in [I::Uninitialized, I::Maybe] {
            let mut storage = Storage::new(I::Initialized);
            storage.set(&hole, state, None);
            for query in &queries {
                check(&storage, query);
            }
        }
    }
}

#[test]
fn symbolic_holes_keep_every_possible_selection_unavailable_without_losing_the_container() {
    let queries = queries();
    let hole = AccessPath::new([
        ValueProjection::Array(IndexFact::Unknown),
        ValueProjection::Array(IndexFact::Known(0)),
        ValueProjection::Tuple(0),
    ]);
    let mut storage = Storage::new(I::Initialized);
    storage.set_path(&hole, I::Uninitialized, None);
    for query in &queries {
        check(&storage, query);
    }
    assert_eq!(storage.present(&FieldPath::default()).state, I::Initialized);
    let mut other = Storage::new(I::Initialized);
    assert!(other.merge(&storage));
    for query in &queries {
        check(&other, query);
    }
    assert!(!other.merge(&storage));
    // A possibly overlapping replacement cannot erase symbolic descendants. Only a proven
    // containing replacement can do so; no equality is assumed for unknown selectors.
    storage.set(
        &FieldPath::new([ValueField::Array(1)]),
        I::Initialized,
        None,
    );
    assert!(
        !storage
            .complete(&FieldPath::default())
            .state
            .eq(&I::Initialized)
    );
    storage.set(&FieldPath::default(), I::Initialized, None);
    assert!(storage.selections.is_empty());
    for query in &queries {
        check(&storage, query);
    }
}
