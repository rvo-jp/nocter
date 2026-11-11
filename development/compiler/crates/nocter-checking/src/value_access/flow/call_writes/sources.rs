//! Shared recursive call-dependency guards, retaining typed/value prerequisites without body IR.
use super::{TypedQuery, requests::Request};
use crate::value_access::{Failure, results::Evidence};
use nocter_declaration_lowering::access_declarations::{BodyDependencies, BodySource, Context};
use nocter_model::ValueCode;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Debug)]
enum Guard {
    Body(BodySource),
    Dependencies(BodyDependencies),
}
#[derive(Debug)]
struct Node {
    guard: Guard,
    results: Option<Arc<Evidence>>,
}
#[derive(Debug, Default)]
pub(super) struct Graph {
    nodes: BTreeMap<ValueCode, Node>,
    dependencies: BTreeMap<Request, BTreeSet<Request>>,
}
impl Graph {
    pub(super) fn new(
        scope: &BTreeMap<ValueCode, Arc<TypedQuery>>,
        dependencies: BTreeMap<Request, BTreeSet<Request>>,
    ) -> Self {
        let nodes = scope
            .iter()
            .map(|(&code, typed)| {
                // The selected body-dependency root includes the body's own source as well as
                // value/count suppliers used to establish its typed operations and data shapes.
                let guard = typed.evaluation_source.as_ref().map_or_else(
                    || Guard::Body(typed.source.clone()),
                    |proof| Guard::Dependencies(proof.clone()),
                );
                (
                    code,
                    Node {
                        guard,
                        results: typed.result_evidence.clone(),
                    },
                )
            })
            .collect();
        Self {
            nodes,
            dependencies,
        }
    }
    pub(super) fn select(
        self: &Arc<Self>,
        caller: ValueCode,
        roots: BTreeSet<Request>,
    ) -> Selected {
        Selected {
            graph: self.clone(),
            caller: Some(caller),
            roots,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Selected {
    graph: Arc<Graph>,
    caller: Option<ValueCode>,
    roots: BTreeSet<Request>,
}
impl Selected {
    pub(super) fn verify(&self, context: &Context<'_>) -> Result<(), Failure> {
        let mut pending = self.roots.clone();
        let mut visited = BTreeSet::new();
        let mut checked: Vec<&Arc<Evidence>> = Vec::new();
        let mut codes = BTreeSet::new();
        while let Some(request) = pending.pop_first() {
            if !visited.insert(request.clone()) {
                continue;
            }
            if let Some(edges) = self.graph.dependencies.get(&request) {
                pending.extend(edges.iter().cloned());
            }
            let code = request.code;
            let Some(node) = self.graph.nodes.get(&code) else {
                continue;
            };
            if Some(code) != self.caller && codes.insert(code) {
                match &node.guard {
                    Guard::Body(source) => {
                        source.bind(context)?;
                    }
                    Guard::Dependencies(proof) => proof.verify(context)?,
                }
                if let Some(proof) = &node.results
                    && !checked.iter().any(|previous| Arc::ptr_eq(previous, proof))
                {
                    proof.verify(context)?;
                    checked.push(proof);
                }
            }
        }
        Ok(())
    }
}
