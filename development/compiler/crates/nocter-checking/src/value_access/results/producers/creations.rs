//! Frozen source creation snapshots, retaining component occurrences before type projection.
use super::{
    INVALID,
    facts::{self, Facts, Node, Store},
};
use crate::value_access::flow::IntegrityError;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{AccessExpressionId, FieldPath, TypeId, ValueClosure, ValueCode};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Possible original producer terms at one exact source creation, never a runtime value.
#[derive(Clone, Debug)]
pub struct ProducerShape {
    nodes: Arc<[Node]>,
    root: Facts,
}
impl ProducerShape {
    #[cfg(test)]
    pub(in crate::value_access::results) fn test_schema(
        program: &nocter_declarations::value_access::Program,
        data: TypeId,
    ) -> Self {
        let mut store = Store::default();
        let root = store.received(program, data, true).unwrap();
        Self {
            nodes: store.freeze(),
            root,
        }
    }
    #[cfg(test)]
    pub(in crate::value_access::results) fn test_components(
        components: impl IntoIterator<Item = (nocter_model::ValueField, FunctionWitness)>,
    ) -> Self {
        let mut store = Store::default();
        let children = components
            .into_iter()
            .map(|(field, witness)| (field, store.single(witness)))
            .collect();
        let root = store.aggregate(children);
        Self {
            nodes: store.freeze(),
            root,
        }
    }
    #[cfg(test)]
    pub(in crate::value_access::results) fn test_terms(
        terms: impl IntoIterator<Item = FunctionWitness>,
    ) -> Self {
        let mut store = Store::default();
        let mut root = Facts::default();
        for witness in terms {
            let term = store.single(witness);
            root = store.join(root, term);
        }
        Self {
            nodes: store.freeze(),
            root,
        }
    }
    pub(in crate::value_access::results) fn identity(&self) -> usize {
        self.root.0
    }
    pub(in crate::value_access::results) fn component(
        &self,
        field: nocter_model::ValueField,
    ) -> Self {
        Self {
            nodes: self.nodes.clone(),
            root: facts::select(&self.nodes, self.root, &[field]),
        }
    }
    /// Query a structural component's possible producers. Arrays use an element summary;
    /// an empty set proves neither an invalid path nor an absent/initialized runtime payload.
    #[must_use]
    pub fn producers_at(&self, fields: &FieldPath) -> &BTreeSet<FunctionWitness> {
        let root = facts::select(&self.nodes, self.root, fields.fields());
        &self.nodes[root.0].producers
    }
}

/// The environment schema and actual producer snapshot at a reached closure-formation operation.
/// This metadata grants no ownership, representation, body capability or source admission.
#[derive(Clone, Debug)]
pub struct CreationRecipe {
    pub owner: ValueCode,
    pub expression: AccessExpressionId,
    pub environment: TypeId,
    pub producers: ProducerShape,
}
struct Draft {
    expression: AccessExpressionId,
    environment: TypeId,
    producers: Facts,
}
#[derive(Default)]
pub(super) struct Creations(BTreeMap<ValueClosure, Draft>);
impl Creations {
    pub(super) fn record(
        &mut self,
        closure: ValueClosure,
        expression: AccessExpressionId,
        environment: TypeId,
        producers: Facts,
        store: &mut Store,
    ) -> Result<(), IntegrityError> {
        let entry = self.0.entry(closure).or_insert(Draft {
            expression,
            environment,
            producers: Facts::default(),
        });
        if entry.expression != expression || entry.environment != environment {
            return Err(INVALID);
        }
        entry.producers = store.join(entry.producers, producers);
        Ok(())
    }
    pub(super) fn finish(
        self,
        owner: ValueCode,
        store: Store,
    ) -> BTreeMap<ValueClosure, CreationRecipe> {
        if self.0.is_empty() {
            return BTreeMap::new();
        }
        let nodes = store.freeze();
        self.0
            .into_iter()
            .map(|(closure, draft)| {
                (
                    closure,
                    CreationRecipe {
                        owner,
                        expression: draft.expression,
                        environment: draft.environment,
                        producers: ProducerShape {
                            nodes: nodes.clone(),
                            root: draft.producers,
                        },
                    },
                )
            })
            .collect()
    }
}
