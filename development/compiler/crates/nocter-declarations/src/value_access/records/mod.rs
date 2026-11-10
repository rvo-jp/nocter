//! Closed by-value record schemas. No source grammar, storage placement, or body state lives here.
use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{
    AccessFieldId, Arena, ArenaBuilder, NominalTypeId, SparseArena, Symbol, SymbolTable, TypeId,
};

use super::data::builtin_copyable;
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

    pub fn validate(self, symbols: &SymbolTable) -> Result<Validated, Vec<DeclarationViolation>> {
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
        let copyable = validate_payloads(&reserved, &mut violations);
        if !violations.is_empty() {
            return Err(violations);
        }
        Ok(Validated {
            reserved,
            fields: self.fields.finish(),
            copyable,
        })
    }
}

/// Construction-only capability: every record has a finite shape and valid explicit Copy policy.
#[derive(Debug)]
pub(super) struct Validated {
    reserved: Arena<NominalTypeId, Reservation>,
    fields: Arena<AccessFieldId, FieldSlot>,
    copyable: BTreeMap<NominalTypeId, bool>,
}

impl Validated {
    pub fn contains(&self, id: NominalTypeId) -> bool {
        self.reserved.get(id).is_some()
    }
}

/// Work from leaves to parents without recursion or expanding a shared schema into payload trees.
/// Each field edge is counted once, including two fields with the same record type.
fn validate_payloads(
    records: &Arena<NominalTypeId, Reservation>,
    violations: &mut Vec<DeclarationViolation>,
) -> BTreeMap<NominalTypeId, bool> {
    let mut remaining = BTreeMap::new();
    let mut parents: BTreeMap<_, Vec<_>> = BTreeMap::new();
    let mut ready = BTreeSet::new();
    for (id, reservation) in records.iter() {
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        let mut count = 0;
        for field in &definition.fields {
            field.data.visit_leaves(|data| {
                if let DataReference::Record(child) = data {
                    parents.entry(*child).or_default().push(id);
                    count += 1;
                }
            });
        }
        remaining.insert(id, count);
        if count == 0 {
            ready.insert(id);
        }
    }
    let mut copyable = BTreeMap::new();
    // Collect field failures first, then report them in reservation/field order, independently of
    // topological traversal order. A failed dependency cannot make a parent's Copy valid.
    let mut noncopy = BTreeSet::new();
    while let Some(id) = ready.pop_first() {
        let definition = records.get(id).unwrap().definition.as_ref().unwrap();
        let mut all_copy = true;
        for (position, field) in definition.fields.iter().enumerate() {
            let mut copy = true;
            field.data.visit_leaves(|data| {
                copy &= match data {
                    DataReference::Builtin(data) => {
                        builtin_copyable(*data).expect("validated builtin payload")
                    }
                    DataReference::Record(child) => copyable[child],
                    DataReference::Tuple(_) => unreachable!("only leaves are visited"),
                };
            });
            all_copy &= copy;
            if definition.copy && !copy {
                noncopy.insert((id, position));
            }
        }
        copyable.insert(id, definition.copy && all_copy);
        for parent in parents.get(&id).into_iter().flatten() {
            let count = remaining.get_mut(parent).unwrap();
            *count -= 1;
            if *count == 0 {
                ready.insert(*parent);
            }
        }
    }
    for (id, reservation) in records.iter() {
        let definition = reservation.definition.as_ref().unwrap();
        if !copyable.contains_key(&id) {
            // Every unsolved record has at least one unsolved child. Retain a concrete field
            // witness, including when this record contains rather than participates in a cycle.
            let (position, through) = definition
                .fields
                .iter()
                .enumerate()
                .find_map(|(position, field)| {
                    let mut through = None;
                    field.data.visit_leaves(|data| {
                        if let DataReference::Record(child) = data
                            && !copyable.contains_key(child)
                            && through.is_none()
                        {
                            through = Some(*child);
                        }
                    });
                    through.map(|child| (position, child))
                })
                .expect("an unresolved payload has an unresolved field");
            violations.push(DeclarationViolation {
                declaration: DeclarationId::Record(id),
                position: HeaderPosition::Field(position),
                rule: DeclarationRule::RecursivePayload { through },
            });
        }
        for position in 0..definition.fields.len() {
            if noncopy.contains(&(id, position)) {
                violations.push(DeclarationViolation {
                    declaration: DeclarationId::Record(id),
                    position: HeaderPosition::Field(position),
                    rule: DeclarationRule::NonCopyField,
                });
            }
        }
    }
    copyable
}
