//! Code facts indexed by callable leaf identity, never by an expanded payload or element index.
//! Equal leaf data denotes one static witness after result equalities close. Facts retain its
//! actual alternative producer terms; neither cardinality nor logical address proves a producer.
use super::{INVALID, Producers};
use crate::value_access::flow::IntegrityError;
use nocter_declarations::value_access::{FunctionWitness, PayloadSchema, Program};
use nocter_model::{TypeId, ValueFunctionOrigin};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Facts(BTreeMap<TypeId, Producers>);
impl Facts {
    pub(super) fn single(data: TypeId, witness: FunctionWitness) -> Self {
        Self(BTreeMap::from([(data, Producers::from([witness]))]))
    }
    pub(super) fn producers(&self, data: TypeId) -> Producers {
        self.0.get(&data).cloned().unwrap_or_default()
    }
    pub(super) fn join(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (&data, witnesses) in &other.0 {
            let entry = self.0.entry(data).or_default();
            let before = entry.len();
            entry.extend(witnesses);
            changed |= before != entry.len();
        }
        changed
    }
    pub(super) fn applications(&self) -> impl Iterator<Item = nocter_model::ValueApplication> + '_ {
        self.0
            .values()
            .flatten()
            .filter_map(|witness| match witness.origin {
                ValueFunctionOrigin::Application(application) => Some(application),
                _ => None,
            })
    }
    pub(super) fn received(
        program: &Program,
        data: TypeId,
        retain_applications: bool,
    ) -> Result<Self, IntegrityError> {
        let mut result = Self::default();
        let mut pending = vec![data];
        let mut seen = BTreeSet::new();
        while let Some(data) = pending.pop() {
            if !seen.insert(data) {
                continue;
            }
            match program.data_schema(data).ok_or(INVALID)? {
                PayloadSchema::Function(witness)
                    if !matches!(witness.origin, ValueFunctionOrigin::Result(_))
                        && (retain_applications
                            || !matches!(witness.origin, ValueFunctionOrigin::Application(_))) =>
                {
                    result.join(&Self::single(data, witness));
                }
                PayloadSchema::Aggregate(schema) => {
                    pending.extend(schema.components().map(|component| component.data));
                }
                PayloadSchema::Array(array) if !array.is_empty() => pending.push(array.element()),
                _ => {}
            }
        }
        Ok(result)
    }
    /// Align an already selected structural boundary. This transports actual facts, not its schema.
    pub(super) fn retag(
        &self,
        program: &Program,
        source: TypeId,
        target: TypeId,
    ) -> Result<Self, IntegrityError> {
        if self.0.is_empty() {
            return Ok(Self::default());
        }
        let mut result = Self::default();
        let mut pending = vec![(source, target)];
        let mut seen = BTreeSet::new();
        while let Some((source, target)) = pending.pop() {
            if !seen.insert((source, target)) {
                continue;
            }
            match (program.data_schema(source), program.data_schema(target)) {
                (Some(PayloadSchema::Function(_)), Some(PayloadSchema::Function(view))) => {
                    for witness in self.producers(source) {
                        if !witness.invocation.can_supply(view.invocation) {
                            return Err(INVALID);
                        }
                        result.join(&Self::single(
                            target,
                            FunctionWitness {
                                invocation: view.invocation,
                                ..witness
                            },
                        ));
                    }
                }
                (Some(PayloadSchema::Array(a)), Some(PayloadSchema::Array(b)))
                    if a.length() == b.length() =>
                {
                    if !a.is_empty() {
                        pending.push((a.element(), b.element()));
                    }
                }
                (Some(PayloadSchema::Aggregate(a)), Some(PayloadSchema::Aggregate(b)))
                    if a.len() == b.len()
                        && (source == target
                            || matches!(
                                (a.form(), b.form()),
                                (
                                    nocter_declarations::value_access::AggregateForm::Tuple(_),
                                    nocter_declarations::value_access::AggregateForm::Tuple(_)
                                )
                            )) =>
                {
                    pending.extend(
                        a.components()
                            .zip(b.components())
                            .map(|(a, b)| (a.data, b.data)),
                    );
                }
                _ if source == target => {}
                _ => return Err(INVALID),
            }
        }
        Ok(result)
    }
    pub(super) fn selected(&self, program: &Program, data: TypeId) -> Result<Self, IntegrityError> {
        self.retag(program, data, data)
    }
}
