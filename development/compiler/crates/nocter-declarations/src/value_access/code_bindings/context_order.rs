//! One iterative structural order over mutually shared callable and data environment contexts.
use super::ConcreteWitness;
use crate::value_access::{DataInstanceKey, data_instances::Context};
use nocter_model::ValueField;
use std::{
    cmp::Ordering,
    collections::{BTreeSet, btree_map},
    sync::Arc,
};

enum Pending<'a> {
    Witness(&'a ConcreteWitness, &'a ConcreteWitness),
    Data(&'a DataInstanceKey, &'a DataInstanceKey),
    Children(
        btree_map::Iter<'a, ValueField, DataInstanceKey>,
        btree_map::Iter<'a, ValueField, DataInstanceKey>,
    ),
}
fn compare(root: Pending<'_>) -> Ordering {
    let mut pending = vec![root];
    let mut compared = BTreeSet::new();
    while let Some(next) = pending.pop() {
        let order = match next {
            Pending::Witness(left, right) => {
                let order = left
                    .code
                    .cmp(&right.code)
                    .then(left.invocation.cmp(&right.invocation));
                if !order.is_eq() {
                    return order;
                }
                match (&left.environment, &right.environment) {
                    (Some(left), Some(right)) => {
                        pending.push(Pending::Data(left, right));
                        Ordering::Equal
                    }
                    (None, None) => Ordering::Equal,
                    (None, Some(_)) => Ordering::Less,
                    (Some(_), None) => Ordering::Greater,
                }
            }
            Pending::Data(left, right) => {
                let order = left.data.cmp(&right.data);
                if !order.is_eq() {
                    return order;
                }
                let pair = (Arc::as_ptr(&left.context), Arc::as_ptr(&right.context));
                if pair.0 == pair.1 || !compared.insert(pair) {
                    continue;
                }
                match (&*left.context, &*right.context) {
                    (Context::Closed, Context::Closed)
                    | (Context::Uninhabited, Context::Uninhabited) => Ordering::Equal,
                    (Context::Closed, _) | (Context::Function(_), Context::Components(_)) => {
                        Ordering::Less
                    }
                    (_, Context::Closed) | (Context::Components(_), Context::Function(_)) => {
                        Ordering::Greater
                    }
                    (Context::Uninhabited, _) => Ordering::Less,
                    (_, Context::Uninhabited) => Ordering::Greater,
                    (Context::Function(left), Context::Function(right)) => {
                        pending.push(Pending::Witness(left, right));
                        Ordering::Equal
                    }
                    (Context::Components(left), Context::Components(right)) => {
                        pending.push(Pending::Children(left.iter(), right.iter()));
                        Ordering::Equal
                    }
                }
            }
            Pending::Children(mut left, mut right) => match (left.next(), right.next()) {
                (Some((lk, lv)), Some((rk, rv))) => {
                    let order = lk.cmp(rk);
                    if !order.is_eq() {
                        return order;
                    }
                    pending.push(Pending::Children(left, right));
                    pending.push(Pending::Data(lv, rv));
                    Ordering::Equal
                }
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
            },
        };
        if !order.is_eq() {
            return order;
        }
    }
    Ordering::Equal
}
impl Ord for ConcreteWitness {
    fn cmp(&self, other: &Self) -> Ordering {
        compare(Pending::Witness(self, other))
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
impl Ord for DataInstanceKey {
    fn cmp(&self, other: &Self) -> Ordering {
        compare(Pending::Data(self, other))
    }
}
impl PartialOrd for DataInstanceKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for DataInstanceKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for DataInstanceKey {}
