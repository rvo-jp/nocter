//! Shared source-only projection of successful query dependencies, without semantic facts.
use super::{BodySource, BodySources, Context, SourceMismatch};
use std::{collections::BTreeSet, sync::Arc};

/// One source requirement and earlier nodes whose requirements it retains.
#[derive(Debug)]
pub struct BodySourceNode {
    pub source: Option<BodySource>,
    pub dependencies: Box<[usize]>,
}

/// One flat source-requirement DAG shared by all selected roots.
/// Construction positions are local graph indices, never semantic identities.
#[derive(Clone, Debug)]
pub struct BodySourceGraph(Arc<[BodySourceNode]>);
impl BodySourceGraph {
    /// Freeze nodes in prerequisite-first order without retaining evaluator values or query state.
    /// # Errors
    /// Rejects unknown, self or forward edges before publishing a graph.
    pub fn new(nodes: Vec<BodySourceNode>) -> Result<Self, SourceMismatch> {
        if nodes
            .iter()
            .enumerate()
            .any(|(index, node)| node.dependencies.iter().any(|&child| child >= index))
        {
            return Err(SourceMismatch::InconsistentDependencies);
        }
        Ok(Self(nodes.into()))
    }
    #[must_use]
    pub fn select(&self, root: usize) -> Option<BodyDependencies> {
        self.0.get(root)?;
        Some(BodyDependencies {
            graph: self.clone(),
            root,
        })
    }
}

/// A conjunction of exact source requirements reachable from one selected query root.
/// Different roots share the graph, but unrelated nodes impose no binding requirement.
#[derive(Clone, Debug)]
pub struct BodyDependencies {
    graph: BodySourceGraph,
    root: usize,
}
impl BodyDependencies {
    /// Verify selected sources against the exact descendant context, once per reached graph node.
    /// # Errors
    /// Rejects changed source or authority through the ordinary paired `BodySource` proof.
    pub fn verify(&self, context: &Context<'_>) -> Result<(), SourceMismatch> {
        for source in self.sources() {
            source.verify_owner(context)?;
        }
        Ok(())
    }
    /// Project just this root's paired guards for one standalone reusable header product.
    /// # Errors
    /// Rejects conflicting source/authority requirements through `BodySources::insert`.
    pub fn collect_sources(&self) -> Result<BodySources, SourceMismatch> {
        let mut sources = BodySources::default();
        for source in self.sources() {
            sources.insert(source)?;
        }
        Ok(sources)
    }
    fn sources(&self) -> impl Iterator<Item = &BodySource> {
        let mut visited = BTreeSet::new();
        let mut pending = vec![self.root];
        std::iter::from_fn(move || {
            while let Some(index) = pending.pop() {
                if !visited.insert(index) {
                    continue;
                }
                let node = &self.graph.0[index];
                pending.extend(node.dependencies.iter().rev().copied());
                if let Some(source) = &node.source {
                    return Some(source);
                }
            }
            None
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access_declarations::{
        Failure, Preparation, Query,
        tests::{bound, input},
    };
    use nocter_syntax::ParseGoal;

    #[test]
    fn selected_roots_ignore_unrelated_sources_and_reject_changed_required_sources() {
        let text = "func a(): usize { 1 }\nfunc b(): usize { 2 }";
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let query = Query::prepare(bound(&sources, &tree)).unwrap();
        let context = query.bind(bound(&sources, &tree)).unwrap();
        let guard = |name| {
            context
                .body_source(
                    context
                        .program()
                        .resolve(context.program().symbols().get(name).unwrap())
                        .unwrap(),
                )
                .unwrap()
        };
        let graph = BodySourceGraph::new(vec![
            BodySourceNode {
                source: Some(guard("a")),
                dependencies: Box::new([]),
            },
            BodySourceNode {
                source: Some(guard("b")),
                dependencies: Box::new([]),
            },
            BodySourceNode {
                source: None,
                dependencies: Box::new([0]),
            },
            BodySourceNode {
                source: None,
                dependencies: Box::new([0, 2]),
            },
        ])
        .unwrap();
        for (before, after, a_valid) in [("{ 1 }", "{ 3 }", false), ("{ 2 }", "{ 3 }", true)] {
            let (next_sources, next_tree) =
                input(&text.replace(before, after), ParseGoal::V2CoreUnit);
            let next = query.bind(bound(&next_sources, &next_tree)).unwrap();
            assert_eq!(graph.select(3).unwrap().verify(&next).is_ok(), a_valid);
            assert_eq!(graph.select(1).unwrap().verify(&next).is_ok(), !a_valid);
        }
        assert!(graph.select(4).is_none());
    }

    #[test]
    fn graph_construction_rejects_non_topological_edges_and_deep_graphs_drop_flatly() {
        for dependencies in [vec![0], vec![1], vec![usize::MAX]] {
            assert!(matches!(
                BodySourceGraph::new(vec![BodySourceNode {
                    source: None,
                    dependencies: dependencies.into()
                }]),
                Err(SourceMismatch::InconsistentDependencies)
            ));
        }
        let (sources, tree) = input("func a(): usize { 1 }", ParseGoal::V2CoreUnit);
        let query = Query::prepare(bound(&sources, &tree)).unwrap();
        let context = query.bind(bound(&sources, &tree)).unwrap();
        let code = context
            .program()
            .resolve(context.program().symbols().get("a").unwrap())
            .unwrap();
        let mut nodes = vec![BodySourceNode {
            source: Some(context.body_source(code).unwrap()),
            dependencies: Box::new([]),
        }];
        for index in 1..32768 {
            nodes.push(BodySourceNode {
                source: None,
                dependencies: Box::new([index - 1]),
            });
        }
        let graph = BodySourceGraph::new(nodes).unwrap();
        graph.select(32767).unwrap().verify(&context).unwrap();
        drop(graph);
    }

    #[test]
    fn header_query_retains_only_selected_suppliers_and_rejects_foreign_evidence() {
        let text = "func a(): usize { 1 }\nfunc b(): usize { 2 }";
        let (sources, tree) = input(text, ParseGoal::V2CoreUnit);
        let syntax = bound(&sources, &tree);
        let mut preparation = Preparation::prepare(syntax).unwrap();
        let early = preparation.current();
        let code = early
            .program()
            .resolve_code(early.program().symbols().get("a").unwrap())
            .unwrap();
        let current = preparation.context(code).unwrap();
        let proof = BodySourceGraph::new(vec![BodySourceNode {
            source: Some(current.body_source(code).unwrap()),
            dependencies: Box::new([]),
        }])
        .unwrap()
        .select(0)
        .unwrap();
        let query = preparation
            .finish_with_dependencies(proof.collect_sources().unwrap())
            .unwrap();
        for (before, after, valid) in [("{ 1 }", "{ 3 }", false), ("{ 2 }", "{ 3 }", true)] {
            let (next_sources, next_tree) =
                input(&text.replace(before, after), ParseGoal::V2CoreUnit);
            assert_eq!(query.bind(bound(&next_sources, &next_tree)).is_ok(), valid);
        }
        assert!(matches!(
            Preparation::prepare(syntax)
                .unwrap()
                .finish_with_dependencies(proof.collect_sources().unwrap()),
            Err(Failure::ChangedHeader)
        ));
    }
}
