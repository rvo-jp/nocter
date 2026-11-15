//! Exact value representation and directed guarded transport share one runtime graph comparison.
use super::RuntimeEnvironment;
use crate::{RuntimeDataId, RuntimeType, RuntimeTypeRepresentation};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    actual: RuntimeDataId,
    expected: RuntimeDataId,
    guarded: bool,
    transport: bool,
}
impl Key {
    fn child(self, actual: RuntimeDataId, expected: RuntimeDataId, guarded: bool) -> Self {
        Self {
            actual,
            expected,
            guarded,
            ..self
        }
    }
}

/// Shared comparison obligations scoped to one immutable runtime environment. This is neither
/// value availability, a source/code-context certificate nor permission to reuse bytes or owners.
pub struct ValueRepresentationRelations<'a> {
    environment: &'a RuntimeEnvironment,
    known: BTreeSet<Key>,
    parents: BTreeMap<Key, BTreeSet<Key>>,
    failed: BTreeSet<Key>,
}
impl RuntimeEnvironment {
    /// Start shared metadata comparisons against this exact immutable environment.
    #[must_use]
    pub fn value_relations(&self) -> ValueRepresentationRelations<'_> {
        ValueRepresentationRelations {
            environment: self,
            known: BTreeSet::new(),
            parents: BTreeMap::new(),
            failed: BTreeSet::new(),
        }
    }
    /// Compare exact represented domains, ignoring only nonexistent empty-array elements.
    #[must_use]
    pub fn has_same_value_representation(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        self.value_relations()
            .has_same_value_representation(actual, expected)
    }
    /// Check directed excluded-alternative transport, not copying, layout or source admission.
    #[must_use]
    pub fn permits_guarded_value_transport(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        self.value_relations()
            .permits_guarded_value_transport(actual, expected)
    }
}
impl ValueRepresentationRelations<'_> {
    /// Preserve represented domains and every positive runtime identity.
    pub fn has_same_value_representation(
        &mut self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        self.compare(Key {
            actual,
            expected,
            guarded: false,
            transport: false,
        })
    }
    /// Permit only guarded excluded alternatives to widen. Empty roots and narrowing reject.
    pub fn permits_guarded_value_transport(
        &mut self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        !matches!(
            self.environment.types().get(actual),
            None | Some(RuntimeType::Uninhabited)
        ) && self.compare(Key {
            actual,
            expected,
            guarded: false,
            transport: true,
        })
    }
    fn compare(&mut self, root: Key) -> bool {
        let mut pending = vec![root];
        let mut failed = Vec::new();
        while let Some(key) = pending.pop() {
            if !self.known.insert(key) {
                continue;
            }
            let Some(children) = self.requirements(key) else {
                failed.push(key);
                continue;
            };
            for child in children {
                self.parents.entry(child).or_default().insert(key);
                if self.failed.contains(&child) {
                    failed.push(key);
                }
                pending.push(child);
            }
        }
        // All obligations are conjunctions. Failure propagates through every dependent parent,
        // including cycles; nodes outside that closure satisfy the same coinductive comparison.
        while let Some(key) = failed.pop() {
            if !self.failed.insert(key) {
                continue;
            }
            if let Some(parents) = self.parents.get(&key) {
                failed.extend(parents);
            }
        }
        !self.failed.contains(&root)
    }
    fn requirements(&self, key: Key) -> Option<Vec<Key>> {
        let environment = self.environment;
        let a = environment.types().get(key.actual)?;
        let b = environment.types().get(key.expected)?;
        if key.actual == key.expected {
            return Some(Vec::new());
        }
        match (a, b) {
            (RuntimeType::Uninhabited, _) if key.transport && key.guarded => Some(Vec::new()),
            (RuntimeType::EmptyArray { .. }, RuntimeType::EmptyArray { .. }) => Some(Vec::new()),
            (
                RuntimeType::FixedArray {
                    element: a,
                    length: al,
                },
                RuntimeType::FixedArray {
                    element: b,
                    length: bl,
                },
            ) if al == bl => Some(vec![key.child(*a, *b, false)]),
            (RuntimeType::Optional(a), RuntimeType::Optional(b))
            | (RuntimeType::Fallible(a), RuntimeType::Fallible(b)) => {
                Some(vec![key.child(*a, *b, true)])
            }
            (RuntimeType::Aggregate, RuntimeType::Aggregate) => {
                let (
                    Some(RuntimeTypeRepresentation::ValueAggregate { fields: a }),
                    Some(RuntimeTypeRepresentation::ValueAggregate { fields: b }),
                ) = (
                    environment.type_representations().get(key.actual),
                    environment.type_representations().get(key.expected),
                )
                else {
                    return None;
                };
                if a.len() < 2 || a.len() != b.len() {
                    return None;
                }
                a.iter()
                    .zip(b.iter())
                    .map(|(a, b)| {
                        (a.field() == b.field()
                            && matches!(a.field(), nocter_model::ValueField::Tuple(_)))
                        .then(|| key.child(a.ty(), b.ty(), false))
                    })
                    .collect()
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
