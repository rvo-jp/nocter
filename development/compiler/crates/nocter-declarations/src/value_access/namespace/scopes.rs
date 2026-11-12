//! Scope construction is consumed once into the immutable name authority.
use super::{Binding, Qualifier, ScopeBindingError, ScopeId};
use crate::value_access::DeclarationId;
use nocter_model::{BuiltinType, ModuleAudience, ModuleIdentity, Symbol, SymbolTable};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug)]
pub(in crate::value_access) struct Scopes {
    pub(super) count: usize,
    pub(super) bindings: Vec<(ScopeId, Symbol, Binding)>,
    pub(super) qualifiers: BTreeMap<(ScopeId, Symbol), Qualifier>,
    pub(super) modules: BTreeMap<ScopeId, ModuleIdentity>,
    pub(super) parents: BTreeMap<ScopeId, ScopeId>,
}
impl Default for Scopes {
    fn default() -> Self {
        Self {
            count: 1,
            bindings: Vec::new(),
            qualifiers: BTreeMap::new(),
            modules: BTreeMap::new(),
            parents: BTreeMap::new(),
        }
    }
}
impl Scopes {
    pub fn contains(&self, scope: ScopeId) -> bool {
        scope.within(self.count)
    }
    pub fn new_scope(&mut self) -> ScopeId {
        let scope = ScopeId::new(self.count);
        self.count = self
            .count
            .checked_add(1)
            .expect("declaration scope domain exhausted");
        scope
    }
    pub fn new_lexical_scope(&mut self, parent: ScopeId) -> Result<ScopeId, ScopeBindingError> {
        if !self.contains(parent) {
            return Err(ScopeBindingError::UnknownScope);
        }
        let child = self.new_scope();
        self.parents.insert(child, parent);
        Ok(child)
    }
    pub fn bind(
        &mut self,
        scope: ScopeId,
        name: Symbol,
        declaration: DeclarationId,
        audience: Option<Arc<ModuleAudience>>,
    ) {
        self.bindings.push((
            scope,
            name,
            Binding {
                declaration,
                canonical: false,
                audience,
            },
        ));
    }
    pub fn set_module(
        &mut self,
        scope: ScopeId,
        module: ModuleIdentity,
    ) -> Result<(), ScopeBindingError> {
        if !self.contains(scope) {
            return Err(ScopeBindingError::UnknownScope);
        }
        if let Some(&parent) = self.parents.get(&scope) {
            let module_scope =
                std::iter::successors(Some(parent), |scope| self.parents.get(scope).copied())
                    .find_map(|scope| self.modules.get(&scope));
            return if module_scope == Some(&module) {
                Ok(())
            } else {
                Err(ScopeBindingError::ConflictingModule)
            };
        }
        match self.modules.entry(scope) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(module);
            }
            std::collections::btree_map::Entry::Occupied(entry) if *entry.get() == module => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(ScopeBindingError::ConflictingModule);
            }
        }
        Ok(())
    }
    pub fn bind_namespace(
        &mut self,
        symbols: &SymbolTable,
        scope: ScopeId,
        name: Symbol,
        target: ScopeId,
        audience: Option<Arc<ModuleAudience>>,
    ) -> Result<(), ScopeBindingError> {
        if !self.contains(scope) || !self.contains(target) {
            return Err(ScopeBindingError::UnknownScope);
        }
        let spelling = symbols
            .spelling(name)
            .ok_or(ScopeBindingError::UnknownSymbol)?;
        if BuiltinType::from_spelling(spelling).is_some() {
            return Err(ScopeBindingError::ReservedBuiltinName);
        }
        let binding = Qualifier { target, audience };
        match self.qualifiers.entry((scope, name)) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(binding);
            }
            std::collections::btree_map::Entry::Occupied(entry)
                if entry.get().target == target
                    && super::same_audience(
                        entry.get().audience.as_deref(),
                        binding.audience.as_deref(),
                    ) => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(ScopeBindingError::ConflictingBinding);
            }
        }
        Ok(())
    }
}
