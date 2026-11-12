//! One owner for independent type/value names, closed before type normalization.
use super::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, Reservation, records,
};
use nocter_model::{
    AccessCodeId, ArenaBuilder, BuiltinType, ModuleAudience, ModuleIdentity, Symbol, SymbolTable,
};
use std::{collections::BTreeMap, sync::Arc};
mod scopes;
pub(super) use scopes::Scopes;

/// Authored type and value names collide only within their own namespace.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Namespace {
    Value,
    Type,
}
/// One closed declaration lookup scope. Identities are local to one reservation/program domain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScopeId(usize);
impl ScopeId {
    /// The sole scope used by the single-source convenience API.
    pub const ROOT: Self = Self(0);
    pub(super) const fn new(index: usize) -> Self {
        Self(index)
    }
    pub(super) const fn within(self, count: usize) -> bool {
        self.0 < count
    }
}
/// Invalid scope/name construction leaves the previous binding unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScopeBindingError {
    UnknownScope,
    UnknownSymbol,
    ReservedBuiltinName,
    ConflictingBinding,
    ConflictingModule,
}
/// A failure position in the supplied finite path, without syntax or source identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualifiedNameError {
    UnknownScope,
    EmptyPath,
    NamespaceOnly { position: usize },
    UnknownNamespace { position: usize },
    UnknownDeclaration { position: usize },
    Inaccessible { position: usize },
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
    declaration: DeclarationId,
    canonical: bool,
    audience: Option<Arc<ModuleAudience>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Qualifier {
    target: ScopeId,
    audience: Option<Arc<ModuleAudience>>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Names {
    count: usize,
    bindings: BTreeMap<(ScopeId, Namespace, Symbol), Binding>,
    qualifiers: BTreeMap<(ScopeId, Symbol), Qualifier>,
    modules: BTreeMap<ScopeId, ModuleIdentity>,
}
impl Names {
    pub fn contains_scope(&self, scope: ScopeId) -> bool {
        scope.0 < self.count
    }
    pub fn get(&self, scope: ScopeId, namespace: Namespace, name: Symbol) -> Option<DeclarationId> {
        Some(self.bindings.get(&(scope, namespace, name))?.declaration)
    }
    pub fn visible_from(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        name: Symbol,
        caller: &ModuleIdentity,
    ) -> Option<DeclarationId> {
        let binding = self.bindings.get(&(scope, namespace, name))?;
        visible(binding.audience.as_deref(), Some(caller)).then_some(binding.declaration)
    }
    pub fn has_namespace(&self, scope: ScopeId, name: Symbol) -> bool {
        self.qualifiers.contains_key(&(scope, name))
    }
    pub fn lookup_path(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: &[Option<Symbol>],
    ) -> Result<DeclarationId, QualifiedNameError> {
        self.walk_path(scope, namespace, path.iter().copied(), false)
            .map(|(declaration, _)| declaration)
    }
    pub fn lookup_prefix(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: &[Option<Symbol>],
    ) -> Result<(DeclarationId, usize), QualifiedNameError> {
        self.walk_path(scope, namespace, path.iter().copied(), true)
    }
    fn walk_path(
        &self,
        scope: ScopeId,
        namespace: Namespace,
        path: impl ExactSizeIterator<Item = Option<Symbol>>,
        permit_components: bool,
    ) -> Result<(DeclarationId, usize), QualifiedNameError> {
        if !self.contains_scope(scope) {
            return Err(QualifiedNameError::UnknownScope);
        }
        let length = path.len();
        if length == 0 {
            return Err(QualifiedNameError::EmptyPath);
        }
        let caller = self.modules.get(&scope);
        let mut current = scope;
        for (position, name) in path.enumerate() {
            let last = position + 1 == length;
            let unknown = || {
                if !last && !permit_components {
                    QualifiedNameError::UnknownNamespace { position }
                } else {
                    QualifiedNameError::UnknownDeclaration { position }
                }
            };
            let name = name.ok_or_else(unknown)?;
            if let Some(qualifier) = self.qualifiers.get(&(current, name)) {
                if !visible(qualifier.audience.as_deref(), caller) {
                    return Err(QualifiedNameError::Inaccessible { position });
                }
                if last {
                    return Err(QualifiedNameError::NamespaceOnly { position });
                }
                current = qualifier.target;
                continue;
            }
            if !last && !permit_components {
                return Err(QualifiedNameError::UnknownNamespace { position });
            }
            let binding = self
                .bindings
                .get(&(current, namespace, name))
                .ok_or_else(unknown)?;
            if !visible(binding.audience.as_deref(), caller) {
                return Err(QualifiedNameError::Inaccessible { position });
            }
            return Ok((binding.declaration, position + 1));
        }
        unreachable!("a nonempty finite path returns at its final component")
    }
    pub fn values(&self) -> impl ExactSizeIterator<Item = &DeclarationId> {
        self.bindings.values().map(|binding| &binding.declaration)
    }
    pub fn declarations(&self) -> impl Iterator<Item = DeclarationId> + '_ {
        self.bindings
            .values()
            .filter_map(|binding| binding.canonical.then_some(binding.declaration))
    }
    pub fn name(&self, declaration: DeclarationId) -> Option<Symbol> {
        self.bindings.iter().find_map(|(&(_, _, name), binding)| {
            (binding.canonical && binding.declaration == declaration).then_some(name)
        })
    }
}

pub(super) fn close(
    symbols: &SymbolTable,
    codes: &ArenaBuilder<AccessCodeId, Reservation>,
    records: &records::Builder,
    scopes: Scopes,
    violations: &mut Vec<DeclarationViolation>,
) -> Names {
    let mut namespace = BTreeMap::new();
    let names = codes
        .iter()
        .filter_map(|(id, reservation)| {
            reservation.name.map(|name| {
                (
                    reservation.scope,
                    name,
                    Binding {
                        declaration: DeclarationId::Code(id),
                        canonical: true,
                        audience: None,
                    },
                )
            })
        })
        .chain(records.names().map(|(id, scope, name)| {
            (
                scope,
                name,
                Binding {
                    declaration: DeclarationId::Record(id),
                    canonical: true,
                    audience: None,
                },
            )
        }))
        .chain(scopes.bindings.iter().cloned());
    for (scope, name, binding) in names {
        let declaration = binding.declaration;
        let kind = match declaration {
            DeclarationId::Code(_) => Namespace::Value,
            DeclarationId::Record(_) => Namespace::Type,
        };
        let rule = match symbols.spelling(name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) if scopes.qualifiers.contains_key(&(scope, name)) => {
                Some(DeclarationRule::NameConflictsWithNamespace { name })
            }
            Some(_) => match namespace.entry((scope, kind, name)) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(binding);
                    None
                }
                std::collections::btree_map::Entry::Occupied(entry)
                    if entry.get().declaration == declaration =>
                {
                    (!same_audience(entry.get().audience.as_deref(), binding.audience.as_deref()))
                        .then_some(DeclarationRule::InconsistentBindingAudience { name })
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    Some(match (declaration, entry.get().declaration) {
                        (DeclarationId::Code(_), DeclarationId::Code(first)) => {
                            DeclarationRule::DuplicateValue { first }
                        }
                        (DeclarationId::Record(_), DeclarationId::Record(first)) => {
                            DeclarationRule::DuplicateRecord { first }
                        }
                        _ => unreachable!("namespace key matches its declaration role"),
                    })
                }
            },
        };
        if let Some(rule) = rule {
            violations.push(DeclarationViolation {
                declaration,
                position: HeaderPosition::Name,
                rule,
            });
        }
    }
    Names {
        count: scopes.count,
        bindings: namespace,
        qualifiers: scopes.qualifiers,
        modules: scopes.modules,
    }
}

fn visible(audience: Option<&ModuleAudience>, caller: Option<&ModuleIdentity>) -> bool {
    audience.is_none_or(|audience| match audience {
        ModuleAudience::Public => true,
        _ => caller.is_some_and(|caller| audience.contains(caller)),
    })
}

fn same_audience(left: Option<&ModuleAudience>, right: Option<&ModuleAudience>) -> bool {
    let left = left.unwrap_or(&ModuleAudience::Public);
    let right = right.unwrap_or(&ModuleAudience::Public);
    left.is_within(right) && right.is_within(left)
}
