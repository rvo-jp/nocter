//! Demand-driven finite-payload and Copy validation before atomic schema publication.
use super::super::data::builtin_copyable;
use super::super::{DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition};
use super::{Builder, DataReference, Field, Record, Schemas, nominal_key};
use nocter_model::{AccessFieldId, NominalTypeId, SparseArenaBuilder, TypeAuthority};

/// One writer for complete transitive payload schemas; nominal leaf keys grant no schema.
#[derive(Debug)]
pub(in crate::value_access) struct Publication {
    records: SparseArenaBuilder<NominalTypeId, Record>,
    fields: SparseArenaBuilder<AccessFieldId, Field>,
}
impl Publication {
    pub fn new(definitions: &Builder, authority: &mut TypeAuthority) -> Self {
        let mut types = authority.transaction();
        for (id, _) in definitions.reservations.iter() {
            types.intern(nominal_key(id)).expect("closed nominal leaf");
        }
        *authority = types.commit(authority).expect("exact header authority");
        Self {
            records: SparseArenaBuilder::new(definitions.reservations.len()),
            fields: SparseArenaBuilder::new(definitions.fields.len()),
        }
    }
    pub fn get(&self, id: NominalTypeId) -> Option<&Record> {
        self.records.get(id)
    }
    pub fn snapshot(&self) -> Schemas {
        Schemas {
            records: self.records.snapshot(),
            fields: self.fields.snapshot(),
        }
    }
    pub fn dependencies(&self, definitions: &Builder, id: NominalTypeId) -> Vec<NominalTypeId> {
        let reservation = definitions
            .reservations
            .get(id)
            .expect("validated record reservation");
        if self.records.get(id).is_some() {
            return Vec::new();
        }
        let definition = reservation
            .definition
            .as_ref()
            .expect("resolved definition");
        let mut children = Vec::new();
        for field in &definition.fields {
            field.data.visit_payload_leaves(|data| {
                if let DataReference::Record(child) = data
                    && self.records.get(*child).is_none()
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
        if self.records.get(id).is_some() {
            return Ok(());
        }
        let reservation = definitions.reservations.get(id).expect("resolved record");
        let definition = reservation
            .definition
            .as_ref()
            .expect("resolved definition");
        // Reject before opening a type transaction or exposing any field. Failed Copy declarations
        // never become ordinary non-Copy schemas that could satisfy a parent's prerequisite.
        if definition.copy {
            let mut violations = Vec::new();
            for (position, field) in definition.fields.iter().enumerate() {
                let mut copy = true;
                field.data.visit_payload_leaves(|data| {
                    copy &= match data {
                        DataReference::Builtin(data) => {
                            builtin_copyable(*data).expect("resolved builtin payload")
                        }
                        DataReference::Record(child) => self
                            .records
                            .get(*child)
                            .expect("ready child schema")
                            .copyable(),
                        DataReference::Tuple(_)
                        | DataReference::Array { .. }
                        | DataReference::Optional { .. } => {
                            unreachable!("only leaves are visited")
                        }
                    };
                });
                if !copy {
                    violations.push(DeclarationViolation {
                        declaration: DeclarationId::Record(id),
                        position: HeaderPosition::Field(position),
                        rule: DeclarationRule::NonCopyField,
                    });
                }
            }
            if !violations.is_empty() {
                return Err(violations);
            }
        }
        let mut types = authority.transaction();
        let mut fields = Vec::new();
        for &field_id in &reservation.fields {
            let slot = definitions.fields.get(field_id).expect("reserved field");
            let field = &definition.fields[slot.position];
            let data = super::super::data::resolved_data(&mut types, &field.data);
            fields.push((
                field_id,
                Field {
                    owner: slot.owner,
                    name: field.name,
                    position: slot.position,
                    data,
                },
            ));
        }
        let record = Record {
            name: reservation.name,
            data: authority
                .store()
                .identity(&nominal_key(id))
                .expect("reserved nominal key"),
            copyable: definition.copy,
            fields: reservation.fields.clone(),
            field_namespace: fields.iter().map(|(id, field)| (field.name, *id)).collect(),
        };
        *authority = types.commit(authority).expect("exact header authority");
        for (id, field) in fields {
            self.fields
                .insert(id, field)
                .expect("define-once reserved field");
        }
        self.records
            .insert(id, record)
            .expect("define-once reserved record");
        Ok(())
    }
}
