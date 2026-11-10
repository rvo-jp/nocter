//! One complete declared-name domain, closed before any type normalization.
use super::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, Reservation, records,
};
use nocter_model::{AccessCodeId, ArenaBuilder, BuiltinType, Symbol, SymbolTable};
use std::collections::BTreeMap;

pub(super) fn close(
    symbols: &SymbolTable,
    codes: &ArenaBuilder<AccessCodeId, Reservation>,
    records: &records::Builder,
    violations: &mut Vec<DeclarationViolation>,
) -> BTreeMap<Symbol, DeclarationId> {
    let mut namespace = BTreeMap::new();
    // Only rejected cross-domain names need a separate first-record diagnostic witness. This
    // cannot resolve a name or enter the accepted namespace.
    let mut record_collisions = BTreeMap::new();
    // Keep first-declaration witnesses within each canonical domain. Cross-domain collisions
    // consistently report the record, independently of physical source order.
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
        let rule = match symbols.spelling(name) {
            None => Some(DeclarationRule::UnknownSymbol),
            Some(name) if BuiltinType::from_spelling(name).is_some() => {
                Some(DeclarationRule::ReservedBuiltinName)
            }
            Some(_) => match namespace.entry(name) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(declaration);
                    None
                }
                std::collections::btree_map::Entry::Occupied(entry) => {
                    Some(match (declaration, *entry.get()) {
                        (DeclarationId::Code(_), DeclarationId::Code(first)) => {
                            DeclarationRule::DuplicateCallable { first }
                        }
                        (DeclarationId::Record(_), DeclarationId::Record(first)) => {
                            DeclarationRule::DuplicateRecord { first }
                        }
                        (DeclarationId::Record(id), DeclarationId::Code(declaration)) => {
                            match record_collisions.entry(name) {
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    entry.insert(id);
                                    DeclarationRule::RecordShadowsCallable { declaration }
                                }
                                std::collections::btree_map::Entry::Occupied(entry) => {
                                    DeclarationRule::DuplicateRecord {
                                        first: *entry.get(),
                                    }
                                }
                            }
                        }
                        (DeclarationId::Code(_), DeclarationId::Record(_)) => {
                            unreachable!("callable reservations precede record reservations")
                        }
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
