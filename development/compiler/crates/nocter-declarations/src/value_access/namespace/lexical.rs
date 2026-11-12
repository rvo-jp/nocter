//! Explicit immutable lexical ancestry; namespace members never inherit from their caller.
use super::{Binding, Names, Namespace, Qualifier, ScopeId};
use nocter_model::Symbol;

pub(super) enum Entry<'a> {
    Declaration(&'a Binding),
    Qualifier(&'a Qualifier),
}
impl Names {
    pub(super) fn lineage(&self, scope: ScopeId) -> impl Iterator<Item = ScopeId> + '_ {
        std::iter::successors(self.contains_scope(scope).then_some(scope), |scope| {
            self.parents.get(scope).copied()
        })
    }
    pub(super) fn entry(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        name: Symbol,
        inherit: bool,
    ) -> Option<Entry<'_>> {
        for candidate in self.lineage(scope) {
            if let Some(qualifier) = self.qualifiers.get(&(candidate, name)) {
                return Some(Entry::Qualifier(qualifier));
            }
            if let Some(binding) = self.bindings.get(&(candidate, namespace, name)) {
                return Some(Entry::Declaration(binding));
            }
            if !inherit {
                break;
            }
        }
        None
    }
}
