//! Demand-driven finite-payload and Copy validation before atomic schema publication.
use super::super::data::builtin_copyable;
use super::super::{DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition};
use super::{
    Builder, DataReference, Enumeration, Field, NominalSchema, Record, ReservationShape, Schemas,
    Variant, nominal_key,
};
use nocter_model::{AccessFieldId, NominalTypeId, SparseArenaBuilder, TypeAuthority, VariantId};

/// One writer for complete transitive payload schemas; nominal leaf keys grant no schema.
#[derive(Debug)]
pub(in crate::value_access) struct Publication {
    nominals: SparseArenaBuilder<NominalTypeId, NominalSchema>,
    fields: SparseArenaBuilder<AccessFieldId, Field>,
    variants: SparseArenaBuilder<VariantId, Variant>,
}
impl Publication {
    pub fn new(definitions: &Builder, authority: &mut TypeAuthority) -> Self {
        let mut types = authority.transaction();
        for (id, _) in definitions.reservations.iter() {
            types.intern(nominal_key(id)).expect("closed nominal leaf");
        }
        *authority = types.commit(authority).expect("exact header authority");
        Self {
            nominals: SparseArenaBuilder::new(definitions.reservations.len()),
            fields: SparseArenaBuilder::new(definitions.fields.len()),
            variants: SparseArenaBuilder::new(definitions.variants.len()),
        }
    }
    pub fn get(&self, id: NominalTypeId) -> Option<&NominalSchema> {
        self.nominals.get(id)
    }
    pub fn snapshot(&self) -> Schemas {
        Schemas {
            types: self.nominals.snapshot(),
            fields: self.fields.snapshot(),
            variants: self.variants.snapshot(),
        }
    }
    pub fn dependencies(&self, definitions: &Builder, id: NominalTypeId) -> Vec<NominalTypeId> {
        let reservation = definitions
            .reservations
            .get(id)
            .expect("validated record reservation");
        if self.nominals.get(id).is_some() {
            return Vec::new();
        }
        let Some(definition) = reservation.record_definition() else {
            return Vec::new();
        };
        let mut children = Vec::new();
        for field in &definition.fields {
            field.data.visit_payload_leaves(|data| {
                if let DataReference::Nominal(child) = data
                    && self.nominals.get(*child).is_none()
                {
                    children.push(*child);
                }
            });
        }
        children
    }
    // The parent writer has established every reported dependency before calling this method.
    pub fn publish_ready(
        &mut self,
        definitions: &Builder,
        id: NominalTypeId,
        authority: &mut TypeAuthority,
    ) -> Result<(), Vec<DeclarationViolation>> {
        if self.nominals.get(id).is_some() {
            return Ok(());
        }
        let reservation = definitions
            .reservations
            .get(id)
            .expect("resolved nominal type");
        let data = authority
            .store()
            .identity(&nominal_key(id))
            .expect("reserved nominal key");
        let (field_ids, definition) = match &reservation.shape {
            ReservationShape::Record {
                fields,
                definition: Some(definition),
            } => (fields, definition),
            ReservationShape::Enumeration {
                variants,
                definition: Some(definition),
            } => {
                let names = &definition.names;
                self.publish_enum(
                    id,
                    Enumeration {
                        name: reservation.name,
                        data,
                        copyable: definition.copyable,
                        variants: variants.clone(),
                        names: names
                            .iter()
                            .copied()
                            .zip(variants.iter().copied())
                            .collect(),
                    },
                    names,
                    definitions,
                );
                return Ok(());
            }
            _ => unreachable!("validated nominal definition"),
        };
        // Reject before opening a type transaction or exposing any field. Failed Copy declarations
        // never become ordinary non-Copy schemas that could satisfy a parent's prerequisite.
        if definition.copy {
            self.check_copy_fields(id, &definition.fields)?;
        }
        let mut types = authority.transaction();
        let mut fields = Vec::new();
        for &field_id in field_ids {
            let slot = definitions.fields.get(field_id).expect("reserved field");
            let field = &definition.fields[slot.position];
            let (data, nested) = super::super::functions::contracts::resolve(
                &mut types,
                nocter_model::ValueContractOwner::Field(field_id),
                &field.data,
            );
            fields.push((
                field_id,
                Field {
                    owner: slot.owner,
                    name: field.name,
                    position: slot.position,
                    data,
                    contracts: nested
                        .into_iter()
                        .map(|(_, contract)| contract)
                        .collect::<Vec<_>>()
                        .into(),
                },
            ));
        }
        let record = Record {
            name: reservation.name,
            data,
            copyable: definition.copy,
            fields: field_ids.clone(),
            field_namespace: fields.iter().map(|(id, field)| (field.name, *id)).collect(),
        };
        *authority = types.commit(authority).expect("exact header authority");
        for (id, field) in fields {
            self.fields
                .insert(id, field)
                .expect("define-once reserved field");
        }
        self.nominals
            .insert(id, NominalSchema::Record(record))
            .expect("define-once reserved record");
        Ok(())
    }

    fn check_copy_fields(
        &self,
        id: NominalTypeId,
        fields: &[super::FieldDefinition],
    ) -> Result<(), Vec<DeclarationViolation>> {
        let mut violations = Vec::new();
        for (position, field) in fields.iter().enumerate() {
            let mut copy = true;
            field.data.visit_payload_leaves(|data| {
                copy &= match data {
                    DataReference::Builtin(data) => {
                        builtin_copyable(*data).expect("resolved builtin payload")
                    }
                    DataReference::Nominal(child) => self
                        .nominals
                        .get(*child)
                        .expect("ready child schema")
                        .copyable(),
                    DataReference::Function(_) | DataReference::Future(_) => false,
                    DataReference::Tuple(_)
                    | DataReference::Array { .. }
                    | DataReference::Optional { .. }
                    | DataReference::Fallible(_) => {
                        unreachable!("only leaves are visited")
                    }
                };
            });
            if !copy {
                violations.push(DeclarationViolation {
                    declaration: DeclarationId::Nominal(id),
                    position: HeaderPosition::Field(position),
                    rule: DeclarationRule::NonCopyField,
                });
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }

    fn publish_enum(
        &mut self,
        id: NominalTypeId,
        schema: Enumeration,
        names: &[nocter_model::Symbol],
        definitions: &Builder,
    ) {
        for (&variant, &name) in schema.variants.iter().zip(names) {
            let slot = definitions.variants.get(variant).expect("reserved variant");
            self.variants
                .insert(
                    variant,
                    Variant {
                        owner: slot.owner,
                        name,
                        position: slot.position,
                    },
                )
                .expect("define-once reserved variant");
        }
        self.nominals
            .insert(id, NominalSchema::Enumeration(schema))
            .expect("define-once reserved enumeration");
    }
}
