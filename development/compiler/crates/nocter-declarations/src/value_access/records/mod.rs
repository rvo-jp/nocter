//! Closed by-value record schemas. No source grammar, storage placement, or body state lives here.
use std::collections::BTreeMap;

use nocter_model::{
    AccessFieldId, ArenaBuilder, NominalTypeId, SparseArena, Symbol, SymbolTable, TypeId,
};

use super::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, data_rule,
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

    pub fn is_defined(&self, id: NominalTypeId) -> Option<bool> {
        Some(self.reservations.get(id)?.definition.is_some())
    }
    pub fn contains(&self, id: NominalTypeId) -> bool {
        self.reservations.get(id).is_some()
    }
    pub fn validate(
        &self,
        id: NominalTypeId,
        symbols: &SymbolTable,
    ) -> Result<(), super::HeaderFailure> {
        let reservation = self
            .reservations
            .get(id)
            .ok_or(super::HeaderFailure::UnknownId)?;
        let mut violations = Vec::new();
        let mut report = |position, rule| {
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Record(id),
                position,
                rule,
            });
        };
        if let Some(definition) = &reservation.definition {
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
                if field.data.has_repeated_failure() {
                    report(position, DeclarationRule::RepeatedFailure);
                }
                field.data.visit_leaves(|data| {
                    if let Some(rule) = data_rule(data, self) {
                        report(position, rule);
                    }
                });
            }
        } else {
            report(HeaderPosition::Name, DeclarationRule::MissingDefinition);
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(super::HeaderFailure::Rejected(violations))
        }
    }
    pub fn cycle_violation(&self, path: &[NominalTypeId]) -> Option<DeclarationViolation> {
        if path.len() < 2 || path.first() != path.last() {
            return None;
        }
        let field_to = |parent, child| {
            self.reservations
                .get(parent)?
                .definition
                .as_ref()?
                .fields
                .iter()
                .position(|field| {
                    let mut found = false;
                    field.data.visit_payload_leaves(|data| {
                        found |= matches!(data, DataReference::Record(id) if *id == child);
                    });
                    found
                })
        };
        let mut closing = None;
        for edge in path.windows(2) {
            closing = Some((edge[0], field_to(edge[0], edge[1])?, edge[1]));
        }
        let (owner, position, through) = closing?;
        Some(DeclarationViolation {
            declaration: DeclarationId::Record(owner),
            position: HeaderPosition::Field(position),
            rule: DeclarationRule::RecursivePayload { through },
        })
    }
}

pub(in crate::value_access) fn nominal_key(id: NominalTypeId) -> nocter_model::TypeKind {
    nocter_model::TypeKind::Nominal {
        definition: id,
        arguments: nocter_model::GenericApplication::default(),
    }
}
