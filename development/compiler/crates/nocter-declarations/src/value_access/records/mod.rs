//! Closed by-value record schemas. No source grammar, storage placement, or body state lives here.
use std::collections::{BTreeMap, BTreeSet};

use nocter_model::{
    AccessCallableId, AccessFieldId, Arena, ArenaBuilder, BuiltinType, GenericApplication,
    NominalTypeId, Symbol, SymbolTable, TypeAuthority, TypeId, TypeKind,
};

use super::data::builtin_copyable;
use super::{
    DataReference, DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, supported,
};
use crate::DefinitionError;

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
    pub records: Arena<NominalTypeId, Record>,
    pub fields: Arena<AccessFieldId, Field>,
    pub namespace: BTreeMap<Symbol, NominalTypeId>,
}

#[derive(Debug)]
struct Definition {
    copy: bool,
    fields: Box<[FieldDefinition]>,
}

#[derive(Debug)]
struct Reservation {
    name: Symbol,
    definition: Option<Definition>,
}

#[derive(Debug, Default)]
pub(super) struct Builder {
    reservations: ArenaBuilder<NominalTypeId, Reservation>,
    namespace: BTreeMap<Symbol, NominalTypeId>,
}

impl Builder {
    pub fn reserve(&mut self, name: Symbol) -> NominalTypeId {
        let id = self.reservations.insert(Reservation {
            name,
            definition: None,
        });
        self.namespace.entry(name).or_insert(id);
        id
    }

    pub fn namespace(&self) -> &BTreeMap<Symbol, NominalTypeId> {
        &self.namespace
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
        reservation.definition = Some(Definition { copy, fields });
        Ok(())
    }

    pub fn validate(
        self,
        symbols: &SymbolTable,
        callables: &BTreeMap<Symbol, AccessCallableId>,
    ) -> Result<Validated, Vec<DeclarationViolation>> {
        let reserved = self.reservations.finish();
        let namespace = self.namespace;
        let mut violations = Vec::new();
        for (id, reservation) in reserved.iter() {
            let mut report = |position, rule| {
                violations.push(DeclarationViolation {
                    declaration: DeclarationId::Record(id),
                    position,
                    rule,
                });
            };
            let rule = match symbols.spelling(reservation.name) {
                None => Some(DeclarationRule::UnknownSymbol),
                Some(name) if BuiltinType::from_spelling(name).is_some() => {
                    Some(DeclarationRule::ReservedBuiltinName)
                }
                Some(_) => {
                    if let Some(&first) = namespace.get(&reservation.name)
                        && first != id
                    {
                        Some(DeclarationRule::DuplicateRecord { first })
                    } else if let Some(&declaration) = callables.get(&reservation.name) {
                        Some(DeclarationRule::RecordShadowsCallable { declaration })
                    } else {
                        None
                    }
                }
            };
            if let Some(rule) = rule {
                report(HeaderPosition::Name, rule);
            }
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
            namespace,
            copyable,
        })
    }
}

/// Construction-only capability: every record has a finite shape and valid explicit Copy policy.
pub(super) struct Validated {
    reserved: Arena<NominalTypeId, Reservation>,
    namespace: BTreeMap<Symbol, NominalTypeId>,
    copyable: BTreeMap<NominalTypeId, bool>,
}

impl Validated {
    pub fn contains(&self, id: NominalTypeId) -> bool {
        self.reserved.get(id).is_some()
    }
    pub fn resolve(&self, name: Symbol) -> Option<NominalTypeId> {
        self.namespace.get(&name).copied()
    }

    pub fn publish(self) -> (TypeAuthority, Schemas) {
        let base = TypeAuthority::new();
        let mut types = base.transaction();
        let data: BTreeMap<_, _> = self
            .reserved
            .iter()
            .map(|(id, _)| {
                let data = types
                    .intern(TypeKind::Nominal {
                        definition: id,
                        arguments: GenericApplication::default(),
                    })
                    .expect("a closed nominal key has no referenced type identities");
                (id, data)
            })
            .collect();
        let mut fields = ArenaBuilder::new();
        let records = self.reserved.map(|id, reservation| {
            let definition = reservation
                .definition
                .as_ref()
                .expect("validated definition");
            let mut field_namespace = BTreeMap::new();
            let field_ids = definition
                .fields
                .iter()
                .enumerate()
                .map(|(position, field)| {
                    let field_id = fields.insert(Field {
                        owner: id,
                        name: field.name,
                        position,
                        data: super::data::resolved_data(
                            &mut types,
                            |record| data[&record],
                            &field.data,
                        ),
                    });
                    field_namespace.insert(field.name, field_id);
                    field_id
                })
                .collect();
            Record {
                name: reservation.name,
                data: data[&id],
                copyable: self.copyable[&id],
                fields: field_ids,
                field_namespace,
            }
        });
        let schemas = Schemas {
            records,
            fields: fields.finish(),
            namespace: self.namespace,
        };
        (
            types
                .commit(&base)
                .expect("exact declaration type authority"),
            schemas,
        )
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
