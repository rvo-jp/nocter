//! Lexical scope identity retained by MIR for cleanup-edge construction.

use super::ids::ScopeId;
use crate::source::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) parent: Option<ScopeId>,
    pub(crate) span: ByteSpan,
}

impl Scope {
    pub(crate) fn root(span: ByteSpan) -> Self {
        Self { parent: None, span }
    }

    pub(crate) fn child(parent: ScopeId, span: ByteSpan) -> Self {
        Self {
            parent: Some(parent),
            span,
        }
    }
}

/// Returns scopes left by an edge in innermost-to-outermost cleanup order.
/// Entered scopes are not included; their locals are initialized by
/// statements after the edge reaches its target block.
pub(crate) fn exited_scopes(scopes: &[Scope], from: ScopeId, to: ScopeId) -> Option<Vec<ScopeId>> {
    scopes.get(from.index())?;
    scopes.get(to.index())?;
    let to_ancestors = ancestors(scopes, to)?.collect::<std::collections::HashSet<_>>();
    let mut exited = Vec::new();
    for scope in ancestors(scopes, from)? {
        if to_ancestors.contains(&scope) {
            return Some(exited);
        }
        exited.push(scope);
    }
    None
}

/// Returns whether `ancestor` contains `scope`, including equality.
///
/// Cleanup construction uses this to distinguish a value that is genuinely
/// live in the source block's lexical scope from stale pre-cleanup dataflow
/// state carried across an edge that already left the value's scope.
pub(crate) fn contains(scopes: &[Scope], ancestor: ScopeId, scope: ScopeId) -> bool {
    ancestors(scopes, scope).is_some_and(|mut scopes| scopes.any(|item| item == ancestor))
}

fn ancestors(scopes: &[Scope], start: ScopeId) -> Option<impl Iterator<Item = ScopeId> + '_> {
    scopes.get(start.index())?;
    let mut next = Some(start);
    Some(std::iter::from_fn(move || {
        let current = next?;
        next = scopes.get(current.index())?.parent;
        Some(current)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceId;

    fn span() -> ByteSpan {
        ByteSpan::new(SourceId::new(0), 0, 1)
    }

    fn tree() -> Vec<Scope> {
        vec![
            Scope::root(span()),
            Scope::child(ScopeId::from_index(0), span()),
            Scope::child(ScopeId::from_index(1), span()),
            Scope::child(ScopeId::from_index(0), span()),
        ]
    }

    #[test]
    fn orders_exited_scopes_from_inner_to_outer() {
        assert_eq!(
            exited_scopes(&tree(), ScopeId::from_index(2), ScopeId::from_index(0)),
            Some(vec![ScopeId::from_index(2), ScopeId::from_index(1)])
        );
    }

    #[test]
    fn crossing_siblings_exits_only_the_source_branch() {
        assert_eq!(
            exited_scopes(&tree(), ScopeId::from_index(2), ScopeId::from_index(3)),
            Some(vec![ScopeId::from_index(2), ScopeId::from_index(1)])
        );
    }

    #[test]
    fn entering_a_child_exits_nothing() {
        assert_eq!(
            exited_scopes(&tree(), ScopeId::from_index(0), ScopeId::from_index(2)),
            Some(Vec::new())
        );
    }

    #[test]
    fn reports_lexical_scope_containment() {
        let scopes = tree();
        assert!(contains(
            &scopes,
            ScopeId::from_index(0),
            ScopeId::from_index(2)
        ));
        assert!(contains(
            &scopes,
            ScopeId::from_index(2),
            ScopeId::from_index(2)
        ));
        assert!(!contains(
            &scopes,
            ScopeId::from_index(2),
            ScopeId::from_index(1)
        ));
        assert!(!contains(
            &scopes,
            ScopeId::from_index(1),
            ScopeId::from_index(3)
        ));
    }
}
