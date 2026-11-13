//! Structural witness order over shared environment DAGs, without recursive expansion.
use super::{ConcreteWitness, ValueParameter};
use std::{
    cmp::Ordering,
    collections::{BTreeSet, btree_map},
};

enum Pending<'a> {
    Witness(&'a ConcreteWitness, &'a ConcreteWitness),
    Captures(
        btree_map::Iter<'a, ValueParameter, ConcreteWitness>,
        btree_map::Iter<'a, ValueParameter, ConcreteWitness>,
    ),
}
impl Ord for ConcreteWitness {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut pending = vec![Pending::Witness(self, other)];
        let mut compared = BTreeSet::new();
        while let Some(next) = pending.pop() {
            match next {
                Pending::Witness(left, right) => {
                    let order = left
                        .code
                        .cmp(&right.code)
                        .then(left.invocation.cmp(&right.invocation));
                    if !order.is_eq() {
                        return order;
                    }
                    let pair = (
                        std::sync::Arc::as_ptr(&left.captures),
                        std::sync::Arc::as_ptr(&right.captures),
                    );
                    if pair.0 == pair.1 || !compared.insert(pair) {
                        continue;
                    }
                    pending.push(Pending::Captures(
                        left.captures.iter(),
                        right.captures.iter(),
                    ));
                }
                Pending::Captures(mut left, mut right) => match (left.next(), right.next()) {
                    (Some((left_key, left_value)), Some((right_key, right_value))) => {
                        let order = left_key.cmp(right_key);
                        if !order.is_eq() {
                            return order;
                        }
                        pending.push(Pending::Captures(left, right));
                        pending.push(Pending::Witness(left_value, right_value));
                    }
                    (Some(_), None) => return Ordering::Greater,
                    (None, Some(_)) => return Ordering::Less,
                    (None, None) => {}
                },
            }
        }
        Ordering::Equal
    }
}
impl PartialOrd for ConcreteWitness {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for ConcreteWitness {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for ConcreteWitness {}
