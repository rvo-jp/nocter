//! One memoization and active-path authority for contracts, typed bodies and required values.
use super::Completed;
use crate::value_access::Query;
use nocter_declarations::value_access::DeclarationId;
use nocter_dependency_query::DependencyQuery;
use nocter_model::ValueCodeRoot;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Request {
    Header(DeclarationId),
    Body(ValueCodeRoot),
    Value(ValueCodeRoot),
}
#[derive(Clone, Debug)]
pub(super) enum Fact {
    Header,
    Body(Box<Query>),
    Value(Completed),
}
pub(super) type Dependencies = DependencyQuery<Request, Fact>;

pub(super) fn body(query: &Dependencies, id: impl Into<ValueCodeRoot>) -> Option<&Query> {
    match query.completed(&Request::Body(id.into()))? {
        Fact::Body(body) => Some(body),
        _ => None,
    }
}
pub(super) fn value(query: &Dependencies, id: impl Into<ValueCodeRoot>) -> Option<&Completed> {
    match query.completed(&Request::Value(id.into()))? {
        Fact::Value(value) => Some(value),
        _ => None,
    }
}
/// Pending header/body work participates in cycle detection, not source execution-call depth.
pub(super) fn value_depth(query: &Dependencies) -> usize {
    query
        .active_keys()
        .filter(|key| matches!(key, Request::Value(_)))
        .count()
}

pub(super) const INVALID: super::Failure =
    super::Failure::Integrity(crate::value_access::flow::IntegrityError::InconsistentTypedBody);

/// Freeze facts once and derive source requirements from the same successful dependency edges.
pub(super) fn finish(
    query: Dependencies,
    context: &nocter_declaration_lowering::access_declarations::Context<'_>,
    condition_limit: std::num::NonZeroU64,
) -> Result<super::Output, super::Failure> {
    use nocter_declaration_lowering::access_declarations as declarations;
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::Arc,
    };
    let mut queries = BTreeMap::new();
    let mut values = BTreeMap::new();
    let mut source_nodes = Vec::new();
    let mut projections: Vec<Option<usize>> = Vec::new();
    let mut body_roots = Vec::new();
    let mut header_roots = BTreeSet::new();
    for node in query.into_dependencies() {
        let source = match node.value.as_ref() {
            Fact::Body(query) => Some(query.source.clone()),
            _ => None,
        };
        let dependencies: Vec<_> = node
            .dependencies
            .iter()
            .filter_map(|&index| projections[index])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        // Semantic facts with no source requirement disappear; one-child conjunctions share that
        // child's projection. Source binding need not walk thousands of source-free header facts.
        let root = if source.is_none() && dependencies.len() <= 1 {
            dependencies.first().copied()
        } else {
            let index = source_nodes.len();
            source_nodes.push(declarations::BodySourceNode {
                source,
                dependencies: dependencies.into(),
            });
            Some(index)
        };
        projections.push(root);
        match (node.key, Arc::unwrap_or_clone(node.value)) {
            (Request::Header(_), Fact::Header) => {
                header_roots.extend(root);
            }
            (Request::Body(id), Fact::Body(query)) => {
                queries.insert(id, *query);
                body_roots.push((id, root.ok_or(INVALID)?));
            }
            (Request::Value(id), Fact::Value(value)) => {
                values.insert(id, Arc::new(value.frozen));
            }
            _ => return Err(INVALID),
        }
    }
    let header_root = source_nodes.len();
    source_nodes.push(declarations::BodySourceNode {
        source: None,
        dependencies: header_roots.into_iter().collect(),
    });
    let sources = declarations::BodySourceGraph::new(source_nodes).map_err(|_| INVALID)?;
    // A standalone header query keeps only selected guards, not the whole Unit's source graph.
    let header_sources = sources
        .select(header_root)
        .ok_or(INVALID)?
        .collect_sources()
        .map_err(|_| INVALID)?;
    header_sources.verify(context).map_err(|_| INVALID)?;
    for (id, root) in body_roots {
        queries
            .get_mut(&id)
            .ok_or(INVALID)?
            .freeze_constant_sources(&sources.select(root).ok_or(INVALID)?);
    }
    Ok(super::Output {
        condition_limit,
        queries,
        values,
        header_sources,
    })
}
