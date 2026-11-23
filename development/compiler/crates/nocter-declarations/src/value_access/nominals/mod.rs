//! Closed by-value nominal schemas. Source grammar, layout and body state live elsewhere.
use std::collections::BTreeMap;

use nocter_model::{
    AccessFieldId, ArenaBuilder, NominalTypeId, SparseArena, Symbol, SymbolTable, TypeId, VariantId,
};

use super::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, ScopeId,
    data_rule,
};
use crate::DefinitionError;

mod publication;
pub(super) use publication::Publication;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalSchema {
    Record(Record),
    Enumeration(Enumeration),
}
impl NominalSchema {
    #[must_use]
    pub const fn data(&self) -> TypeId {
        match self {
            Self::Record(record) => record.data(),
            Self::Enumeration(schema) => schema.data(),
        }
    }
    #[must_use]
    pub const fn name(&self) -> Symbol {
        match self {
            Self::Record(record) => record.name(),
            Self::Enumeration(schema) => schema.name(),
        }
    }
    #[must_use]
    pub const fn copyable(&self) -> bool {
        match self {
            Self::Record(record) => record.copyable(),
            Self::Enumeration(schema) => schema.copyable(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enumeration {
    name: Symbol,
    data: TypeId,
    copyable: bool,
    variants: Box<[VariantId]>,
    names: BTreeMap<Symbol, VariantId>,
}
impl Enumeration {
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
    #[must_use]
    pub const fn copyable(&self) -> bool {
        self.copyable
    }
    #[must_use]
    pub const fn variants(&self) -> &[VariantId] {
        &self.variants
    }
    #[must_use]
    pub fn resolve_variant(&self, name: Symbol) -> Option<VariantId> {
        self.names.get(&name).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Variant {
    owner: NominalTypeId,
    name: Symbol,
    position: usize,
}
impl Variant {
    #[must_use]
    pub const fn owner(&self) -> NominalTypeId {
        self.owner
    }
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    owner: NominalTypeId,
    name: Symbol,
    position: usize,
    data: TypeId,
    contracts: std::sync::Arc<[super::FunctionConstraint]>,
}

impl Field {
    pub(super) fn callable_contracts(&self) -> &[super::FunctionConstraint] {
        &self.contracts
    }
    #[must_use]
    pub const fn owner(&self) -> NominalTypeId {
        self.owner
    }
    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }
    #[must_use]
    pub const fn position(&self) -> usize {
        self.position
    }
    #[must_use]
    pub const fn data(&self) -> TypeId {
        self.data
    }
}

#[derive(Clone, Debug)]
pub(super) struct Schemas {
    pub types: SparseArena<NominalTypeId, NominalSchema>,
    pub fields: SparseArena<AccessFieldId, Field>,
    pub variants: SparseArena<VariantId, Variant>,
}

#[derive(Debug)]
struct RecordDefinition {
    copy: bool,
    fields: Box<[FieldDefinition]>,
}
#[derive(Debug)]
struct EnumDefinition {
    copyable: bool,
    names: Box<[Symbol]>,
}

#[derive(Debug)]
struct Reservation {
    scope: ScopeId,
    name: Symbol,
    shape: ReservationShape,
}
impl Reservation {
    fn record_definition(&self) -> Option<&RecordDefinition> {
        let ReservationShape::Record { definition, .. } = &self.shape else {
            return None;
        };
        definition.as_ref()
    }
}

#[derive(Debug)]
enum ReservationShape {
    Record {
        fields: Box<[AccessFieldId]>,
        definition: Option<RecordDefinition>,
    },
    Enumeration {
        variants: Box<[VariantId]>,
        definition: Option<EnumDefinition>,
    },
}

#[derive(Clone, Copy, Debug)]
struct MemberSlot {
    owner: NominalTypeId,
    position: usize,
}

#[derive(Debug, Default)]
pub(super) struct Builder {
    reservations: ArenaBuilder<NominalTypeId, Reservation>,
    fields: ArenaBuilder<AccessFieldId, MemberSlot>,
    variants: ArenaBuilder<VariantId, MemberSlot>,
}

impl Builder {
    pub fn reserve(&mut self, scope: ScopeId, name: Symbol, field_count: usize) -> NominalTypeId {
        let owner = self.reservations.next_id();
        let fields = (0..field_count)
            .map(|position| self.fields.insert(MemberSlot { owner, position }))
            .collect();
        self.reservations.insert(Reservation {
            scope,
            name,
            shape: ReservationShape::Record {
                fields,
                definition: None,
            },
        })
    }

    pub fn reserve_enum(&mut self, scope: ScopeId, name: Symbol, count: usize) -> NominalTypeId {
        let owner = self.reservations.next_id();
        let variants = (0..count)
            .map(|position| self.variants.insert(MemberSlot { owner, position }))
            .collect();
        self.reservations.insert(Reservation {
            scope,
            name,
            shape: ReservationShape::Enumeration {
                variants,
                definition: None,
            },
        })
    }

    pub fn has_same_reserved_layout(&self, retained: &Schemas) -> bool {
        self.reservations.len() == retained.types.len()
            && self.fields.len() == retained.fields.len()
            && self.variants.len() == retained.variants.len()
            && self.reservations.iter().all(|(id, slot)| {
                retained.types.get(id).is_some_and(|schema| {
                    slot.name == schema.name()
                        && match (&slot.shape, schema) {
                            (
                                ReservationShape::Record { fields, .. },
                                NominalSchema::Record(record),
                            ) => *fields == record.fields,
                            (
                                ReservationShape::Enumeration { variants, .. },
                                NominalSchema::Enumeration(schema),
                            ) => *variants == schema.variants,
                            _ => false,
                        }
                })
            })
            && self.fields.iter().all(|(id, slot)| {
                retained.fields.get(id).is_some_and(|field| {
                    field.owner == slot.owner && field.position == slot.position
                })
            })
            && self.variants.iter().all(|(id, slot)| {
                retained.variants.get(id).is_some_and(|variant| {
                    variant.owner == slot.owner && variant.position == slot.position
                })
            })
    }
    pub fn scope(&self, id: NominalTypeId) -> Option<ScopeId> {
        Some(self.reservations.get(id)?.scope)
    }

    pub fn field_ids(&self, id: NominalTypeId) -> Option<&[AccessFieldId]> {
        match &self.reservations.get(id)?.shape {
            ReservationShape::Record { fields, .. } => Some(fields),
            ReservationShape::Enumeration { .. } => None,
        }
    }

    pub fn names(&self) -> impl Iterator<Item = (NominalTypeId, ScopeId, Symbol)> {
        self.reservations
            .iter()
            .map(|(id, reservation)| (id, reservation.scope, reservation.name))
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
        let ReservationShape::Record {
            fields: ids,
            definition,
        } = &mut reservation.shape
        else {
            return Err(DefinitionError::UnknownId);
        };
        if definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        if fields.len() != ids.len() {
            return Err(DefinitionError::WrongArity {
                expected: ids.len(),
                actual: fields.len(),
            });
        }
        *definition = Some(RecordDefinition { copy, fields });
        Ok(())
    }

    pub fn define_enum(
        &mut self,
        id: NominalTypeId,
        copyable: bool,
        variant_names: Box<[Symbol]>,
    ) -> Result<(), DefinitionError> {
        let reservation = self
            .reservations
            .get_mut(id)
            .ok_or(DefinitionError::UnknownId)?;
        let ReservationShape::Enumeration {
            variants,
            definition,
        } = &mut reservation.shape
        else {
            return Err(DefinitionError::UnknownId);
        };
        if definition.is_some() {
            return Err(DefinitionError::AlreadyDefined);
        }
        if variant_names.len() != variants.len() {
            return Err(DefinitionError::WrongArity {
                expected: variants.len(),
                actual: variant_names.len(),
            });
        }
        *definition = Some(EnumDefinition {
            copyable,
            names: variant_names,
        });
        Ok(())
    }

    pub fn is_defined(&self, id: NominalTypeId) -> Option<bool> {
        Some(match &self.reservations.get(id)?.shape {
            ReservationShape::Record { definition, .. } => definition.is_some(),
            ReservationShape::Enumeration { definition, .. } => definition.is_some(),
        })
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
                declaration: DeclarationId::Nominal(id),
                position,
                rule,
            });
        };
        if let Some(definition) = reservation.record_definition() {
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
                    if !matches!(data, DataReference::Function(_))
                        && let Some(rule) = data_rule(data, self)
                    {
                        report(position, rule);
                    }
                });
            }
        } else if let ReservationShape::Enumeration {
            definition: Some(definition),
            ..
        } = &reservation.shape
        {
            let names = &definition.names;
            if names.is_empty() {
                report(HeaderPosition::Name, DeclarationRule::EmptyEnumeration);
            }
            let mut seen = BTreeMap::new();
            for (index, &name) in names.iter().enumerate() {
                let position = HeaderPosition::Variant(index);
                if symbols.spelling(name).is_none() {
                    report(position, DeclarationRule::UnknownSymbol);
                } else if let Some(&first) = seen.get(&name) {
                    report(position, DeclarationRule::DuplicateVariant { first });
                } else {
                    seen.insert(name, index);
                }
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
                .record_definition()?
                .fields
                .iter()
                .position(|field| {
                    let mut found = false;
                    field.data.visit_payload_leaves(|data| {
                        found |= matches!(data, DataReference::Nominal(id) if *id == child);
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
            declaration: DeclarationId::Nominal(owner),
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
