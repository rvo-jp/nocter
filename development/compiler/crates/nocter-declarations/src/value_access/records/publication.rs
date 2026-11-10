//! Demand-driven finite-payload and Copy validation before atomic schema publication.
use super::super::data::builtin_copyable;
use super::super::{DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition};
use super::{DataReference, DefinitionError, Field, Record, Resolved, Schemas};
use nocter_model::{
    AccessFieldId, GenericApplication, NominalTypeId, SparseArenaBuilder, TypeAuthority, TypeKind,
};

/// One writer for complete transitive payload schemas; nominal leaf keys grant no schema.
#[derive(Debug)]
pub(in crate::value_access) struct Publication {
    resolved: Resolved,
    records: SparseArenaBuilder<NominalTypeId, Record>,
    fields: SparseArenaBuilder<AccessFieldId, Field>,
}
impl Publication {
    pub fn new(resolved: Resolved, authority: &mut TypeAuthority) -> Self {
        let mut types = authority.transaction();
        for (id, _) in resolved.reserved.iter() {
            types.intern(nominal_key(id)).expect("closed nominal leaf");
        }
        *authority = types.commit(authority).expect("exact header authority");
        Self {
            records: SparseArenaBuilder::new(resolved.reserved.len()),
            fields: SparseArenaBuilder::new(resolved.fields.len()),
            resolved,
        }
    }
    pub fn ids(&self) -> impl Iterator<Item = NominalTypeId> + '_ {
        self.resolved.reserved.iter().map(|(id, _)| id)
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
            .resolved
            .reserved
            .get(id)
            .ok_or(DefinitionError::UnknownId)?;
        if self.records.get(id).is_some() {
            return Ok(Vec::new());
        }
        let definition = reservation
            .definition
            .as_ref()
            .expect("resolved definition");
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
    pub fn publish_ready(
        &mut self,
        id: NominalTypeId,
        authority: &mut TypeAuthority,
    ) -> Result<(), Vec<DeclarationViolation>> {
        if self.records.get(id).is_some() {
            return Ok(());
        }
        let reservation = self.resolved.reserved.get(id).expect("resolved record");
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
                field.data.visit_leaves(|data| {
                    copy &= match data {
                        DataReference::Builtin(data) => {
                            builtin_copyable(*data).expect("resolved builtin payload")
                        }
                        DataReference::Record(child) => self
                            .records
                            .get(*child)
                            .expect("ready child schema")
                            .copyable(),
                        DataReference::Tuple(_) => unreachable!("only leaves are visited"),
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
            let slot = self.resolved.fields.get(field_id).expect("reserved field");
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

    pub fn cycle_violation(&self, path: &[NominalTypeId]) -> Option<DeclarationViolation> {
        if path.len() < 2 || path.first() != path.last() {
            return None;
        }
        let field_to = |parent, child| {
            self.resolved
                .reserved
                .get(parent)?
                .definition
                .as_ref()?
                .fields
                .iter()
                .position(|field| {
                    let mut found = false;
                    field.data.visit_leaves(|data| {
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

fn nominal_key(id: NominalTypeId) -> TypeKind {
    TypeKind::Nominal {
        definition: id,
        arguments: GenericApplication::default(),
    }
}
