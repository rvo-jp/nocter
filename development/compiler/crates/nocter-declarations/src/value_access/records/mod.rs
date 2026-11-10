//! Closed by-value record schemas. No source grammar, storage placement, or body state lives here.
use std::collections::BTreeMap;

use nocter_model::{
    AccessFieldId, Arena, ArenaBuilder, NominalTypeId, SparseArena, Symbol, SymbolTable, TypeId,
};

use super::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, supported,
};
use crate::DefinitionError;

mod publication;
pub(super) use publication::Publication;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDefinition {
    pub name: Symbol,
    pub data: DataReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    name: Symbol,
    data: TypeId,
    copyable: bool,
    fields: Box<[AccessFieldId]>,
    field_namespace: BTreeMap<Symbol, AccessFieldId>,
}

impl Record {
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
    /// True only for an explicitly Copy record whose complete payload has Copy evidence.
    #[must_use]
    pub const fn copyable(&self) -> bool {
        self.copyable
    }
    /// Authored declaration order. Implicit partial cleanup visits surviving fields in reverse.
    #[must_use]
    pub const fn fields(&self) -> &[AccessFieldId] {
        &self.fields
    }
    #[must_use]
    pub fn resolve_field(&self, name: Symbol) -> Option<AccessFieldId> {
        self.field_namespace.get(&name).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field {
    owner: NominalTypeId,
    name: Symbol,
    position: usize,
    data: TypeId,
}

impl Field {
    #[must_use]
    pub const fn owner(self) -> NominalTypeId {
        self.owner
    }
    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn position(self) -> usize {
        self.position
    }
    #[must_use]
    pub const fn data(self) -> TypeId {
        self.data
    }
}

#[derive(Clone, Debug)]
pub(super) struct Schemas {
    pub records: SparseArena<NominalTypeId, Record>,
    pub fields: SparseArena<AccessFieldId, Field>,
}

#[derive(Debug)]
struct Definition {
    copy: bool,
    fields: Box<[FieldDefinition]>,
}

#[derive(Debug)]
struct Reservation {
    name: Symbol,
    fields: Box<[AccessFieldId]>,
    definition: Option<Definition>,
}

#[derive(Clone, Copy, Debug)]
struct FieldSlot {
    owner: NominalTypeId,
    position: usize,
}

#[derive(Debug, Default)]
pub(super) struct Builder {
    reservations: ArenaBuilder<NominalTypeId, Reservation>,
    fields: ArenaBuilder<AccessFieldId, FieldSlot>,
}

impl Builder {
    pub fn reserve(&mut self, name: Symbol, field_count: usize) -> NominalTypeId {
        let owner = self.reservations.next_id();
        let fields = (0..field_count)
            .map(|position| self.fields.insert(FieldSlot { owner, position }))
            .collect();
        self.reservations.insert(Reservation {
            name,
            fields,
            definition: None,
        })
    }

    pub fn field_ids(&self, id: NominalTypeId) -> Option<&[AccessFieldId]> {
        self.reservations
            .get(id)
            .map(|reserved| reserved.fields.as_ref())
    }

    pub fn names(&self) -> impl Iterator<Item = (NominalTypeId, Symbol)> {
        self.reservations
            .iter()
            .map(|(id, reservation)| (id, reservation.name))
    }

    pub fn define(
        &mut self,
        id: NominalTypeId,
        copy: bool,
        fields: Box<[FieldDefinition]>,
    ) -> Result<(), DefinitionError> {
        let reservation = self
            .reservations
            .get_mut(id)
            .ok_or(DefinitionError::UnknownId)?;
        if reservation.definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        if fields.len() != reservation.fields.len() {
            return Err(DefinitionError::WrongArity {
                expected: reservation.fields.len(),
                actual: fields.len(),
            });
        }
        reservation.definition = Some(Definition { copy, fields });
        Ok(())
    }

    pub fn validate(self, symbols: &SymbolTable) -> Result<Resolved, Vec<DeclarationViolation>> {
        let reserved = self.reservations.finish();
        let mut violations = Vec::new();
        for (id, reservation) in reserved.iter() {
            let mut report = |position, rule| {
                violations.push(DeclarationViolation {
                    declaration: DeclarationId::Record(id),
                    position,
                    rule,
                });
            };
            let Some(definition) = &reservation.definition else {
                report(HeaderPosition::Name, DeclarationRule::MissingDefinition);
                continue;
            };
            let mut seen = BTreeMap::new();
            for (index, field) in definition.fields.iter().enumerate() {
                let position = HeaderPosition::Field(index);
                if symbols.spelling(field.name).is_none() {
                    report(position, DeclarationRule::UnknownSymbol);
                } else if let Some(&first) = seen.get(&field.name) {
                    report(position, DeclarationRule::DuplicateField { first });
                } else {
                    seen.insert(field.name, index);
                }
                field.data.visit_leaves(|data| match data {
                    DataReference::Builtin(data) if !supported(*data) => report(
                        position,
                        DeclarationRule::UnsupportedDataType { data: *data },
                    ),
                    DataReference::Record(record) if reserved.get(*record).is_none() => report(
                        position,
                        DeclarationRule::UnknownRecordType { record: *record },
                    ),
                    _ => {}
                });
            }
        }
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(Resolved {
            reserved,
            fields: self.fields.finish(),
        })
    }
}

/// Closed raw definitions with resolved names; payload and Copy evidence is established on demand.
#[derive(Debug)]
pub(super) struct Resolved {
    reserved: Arena<NominalTypeId, Reservation>,
    fields: Arena<AccessFieldId, FieldSlot>,
}

impl Resolved {
    pub fn contains(&self, id: NominalTypeId) -> bool {
        self.reserved.get(id).is_some()
    }
}
