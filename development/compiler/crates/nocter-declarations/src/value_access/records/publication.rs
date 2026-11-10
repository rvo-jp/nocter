//! Demand-driven normalization over validated finite record definitions.
use super::{DataReference, DefinitionError, Field, Record, Schemas, Validated};
use nocter_model::{
    AccessFieldId, GenericApplication, NominalTypeId, SparseArenaBuilder, TypeAuthority, TypeKind,
};

/// One writer for complete transitive payload schemas; nominal leaf keys grant no schema.
#[derive(Debug)]
pub(in crate::value_access) struct Publication {
    validated: Validated,
    records: SparseArenaBuilder<NominalTypeId, Record>,
    fields: SparseArenaBuilder<AccessFieldId, Field>,
}
impl Publication {
    pub fn new(validated: Validated, authority: &mut TypeAuthority) -> Self {
        let mut types = authority.transaction();
        for (id, _) in validated.reserved.iter() {
            types.intern(nominal_key(id)).expect("closed nominal leaf");
        }
        *authority = types.commit(authority).expect("exact header authority");
        Self {
            records: SparseArenaBuilder::new(validated.reserved.len()),
            fields: SparseArenaBuilder::new(validated.fields.len()),
            validated,
        }
    }
    pub fn ids(&self) -> impl Iterator<Item = NominalTypeId> + '_ {
        self.validated.reserved.iter().map(|(id, _)| id)
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
    pub fn dependencies(&self, id: NominalTypeId) -> Result<Vec<NominalTypeId>, DefinitionError> {
        let reservation = self
            .validated
            .reserved
            .get(id)
            .ok_or(DefinitionError::UnknownId)?;
        if self.records.get(id).is_some() {
            return Ok(Vec::new());
        }
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        let mut children = Vec::new();
        for field in &definition.fields {
            field.data.visit_leaves(|data| {
                if let DataReference::Record(child) = data
                    && self.records.get(*child).is_none()
                {
                    children.push(*child);
                }
            });
        }
        Ok(children)
    }
    // The parent writer has established every reported dependency before calling this method.
    pub fn publish_ready(&mut self, id: NominalTypeId, authority: &mut TypeAuthority) {
        if self.records.get(id).is_some() {
            return;
        }
        let reservation = self.validated.reserved.get(id).expect("validated record");
        let definition = reservation
            .definition
            .as_ref()
            .expect("validated definition");
        let mut types = authority.transaction();
        let mut fields = Vec::new();
        for &field_id in &reservation.fields {
            let slot = self.validated.fields.get(field_id).expect("reserved field");
            let field = &definition.fields[slot.position];
            let data = super::super::data::resolved_data(
                &mut types,
                |child| self.records.get(child).expect("ready child schema").data(),
                &field.data,
            );
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
            copyable: self.validated.copyable[&id],
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
    }
}

fn nominal_key(id: NominalTypeId) -> TypeKind {
    TypeKind::Nominal {
        definition: id,
        arguments: GenericApplication::default(),
    }
}
