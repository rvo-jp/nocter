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
    pub fn publish(
        &mut self,
        id: NominalTypeId,
        authority: &mut TypeAuthority,
    ) -> Result<(), DefinitionError> {
        if !self.validated.contains(id) {
            return Err(DefinitionError::UnknownId);
        }
        // Validation has proved finite containment. Postorder visits shared children once and
        // never expands a represented payload or uses the host call stack for nominal depth.
        let mut pending = vec![(id, false)];
        while let Some((id, expanded)) = pending.pop() {
            if self.records.get(id).is_some() {
                continue;
            }
            let reservation = self.validated.reserved.get(id).expect("validated record");
            let definition = reservation
                .definition
                .as_ref()
                .expect("validated definition");
            if !expanded {
                pending.push((id, true));
                let mut children = Vec::new();
                for field in &definition.fields {
                    field.data.visit_leaves(|data| {
                        if let DataReference::Record(child) = data {
                            children.push((*child, false));
                        }
                    });
                }
                pending.extend(children.into_iter().rev());
                continue;
            }
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
        Ok(())
    }
}
fn nominal_key(id: NominalTypeId) -> TypeKind {
    TypeKind::Nominal {
        definition: id,
        arguments: GenericApplication::default(),
    }
}
