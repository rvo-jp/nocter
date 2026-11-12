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
pub(super) type Names = BTreeMap<(Namespace, Symbol), DeclarationId>;

pub(super) fn close(
    symbols: &SymbolTable,
    codes: &ArenaBuilder<AccessCodeId, Reservation>,
    records: &records::Builder,
    violations: &mut Vec<DeclarationViolation>,
) -> Names {
    let mut namespace = BTreeMap::new();
    let names = codes
        .iter()
        .filter_map(|(id, reservation)| {
            reservation.name.map(|name| (DeclarationId::Code(id), name))
        })
        .chain(
            records
                .names()
                .map(|(id, name)| (DeclarationId::Record(id), name)),
        );
    for (declaration, name) in names {
        let kind = match declaration {
            DeclarationId::Code(_) => Namespace::Value,
            DeclarationId::Record(_) => Namespace::Type,
        };
        let rule = match symbols.spelling(name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => match namespace.entry((kind, name)) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(declaration);
                    None
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    Some(match (declaration, *entry.get()) {
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
    namespace
}
