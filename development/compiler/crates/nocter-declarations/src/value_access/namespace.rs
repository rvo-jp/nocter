//! One owner for independent type/value names, closed before type normalization.
use super::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, Reservation, records,
};
use nocter_model::{AccessCodeId, ArenaBuilder, BuiltinType, Symbol, SymbolTable};
use std::collections::BTreeMap;

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
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Names {
    count: usize,
    bindings: BTreeMap<(ScopeId, Namespace, Symbol), (DeclarationId, bool)>,
}
impl Names {
    pub fn contains_scope(&self, scope: ScopeId) -> bool {
        scope.0 < self.count
    }
    pub fn get(&self, scope: ScopeId, namespace: Namespace, name: Symbol) -> Option<DeclarationId> {
        Some(self.bindings.get(&(scope, namespace, name))?.0)
    }
    pub fn values(&self) -> impl ExactSizeIterator<Item = &DeclarationId> {
        self.bindings.values().map(|(declaration, _)| declaration)
    }
    pub fn declarations(&self) -> impl Iterator<Item = DeclarationId> + '_ {
        self.bindings
            .values()
            .filter_map(|&(declaration, canonical)| canonical.then_some(declaration))
    }
    pub fn name(&self, declaration: DeclarationId) -> Option<Symbol> {
        self.bindings
            .iter()
            .find_map(|(&(_, _, name), &(id, canonical))| {
                (canonical && id == declaration).then_some(name)
            })
    }
}

pub(super) fn close(
    symbols: &SymbolTable,
    codes: &ArenaBuilder<AccessCodeId, Reservation>,
    records: &records::Builder,
    scope_count: usize,
    bindings: &[(ScopeId, Symbol, DeclarationId)],
    violations: &mut Vec<DeclarationViolation>,
) -> Names {
    let mut namespace = BTreeMap::new();
    let names = codes
        .iter()
        .filter_map(|(id, reservation)| {
            reservation
                .name
                .map(|name| (DeclarationId::Code(id), reservation.scope, name, true))
        })
        .chain(
            records
                .names()
                .map(|(id, scope, name)| (DeclarationId::Record(id), scope, name, true)),
        )
        .chain(
            bindings
                .iter()
                .map(|&(scope, name, declaration)| (declaration, scope, name, false)),
        );
    for (declaration, scope, name, canonical) in names {
        let kind = match declaration {
            DeclarationId::Code(_) => Namespace::Value,
            DeclarationId::Record(_) => Namespace::Type,
        };
        let rule = match symbols.spelling(name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => match namespace.entry((scope, kind, name)) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((declaration, canonical));
                    None
                }
                std::collections::btree_map::Entry::Occupied(entry)
                    if entry.get().0 == declaration =>
                {
                    None
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    Some(match (declaration, entry.get().0) {
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
        count: scope_count,
        bindings: namespace,
    }
}
