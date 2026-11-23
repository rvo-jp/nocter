//! Ordered compile-time slots, independent of typed header and runtime input readiness.
use crate::{DefinitionError, GenericParameter, GenericParameterDomain};
use nocter_model::{
    AccessCodeId, Arena, ArenaBuilder, GenericParameterId, Symbol, TypeKind, TypeTransaction,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub(super) struct Reservations {
    by_code: BTreeMap<AccessCodeId, Box<[GenericParameterId]>>,
    slots: ArenaBuilder<GenericParameterId, GenericParameter<AccessCodeId>>,
}

impl Reservations {
    pub(super) fn named(
        &self,
        code: AccessCodeId,
        name: Symbol,
    ) -> Option<(GenericParameterId, GenericParameter<AccessCodeId>)> {
        named(self.by_code.get(&code)?, name, |id| {
            self.slots.get(id).copied()
        })
    }

    pub(super) fn reserve(
        &mut self,
        code: AccessCodeId,
        parameters: Vec<(Symbol, GenericParameterDomain)>,
    ) -> Result<(), DefinitionError> {
        let std::collections::btree_map::Entry::Vacant(entry) = self.by_code.entry(code) else {
            return Err(DefinitionError::AlreadyDefined);
        };
        let mut names = BTreeSet::new();
        if !parameters.iter().all(|(name, _)| names.insert(*name)) {
            return Err(DefinitionError::AlreadyDefined);
        }
        if !parameters.is_empty() {
            entry.insert(
                parameters
                    .into_iter()
                    .enumerate()
                    .map(|(position, (name, domain))| {
                        self.slots
                            .insert(GenericParameter::new(code, name, position, domain))
                    })
                    .collect(),
            );
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Parameters {
        Parameters {
            by_code: self.by_code,
            slots: self.slots.finish(),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Parameters {
    by_code: BTreeMap<AccessCodeId, Box<[GenericParameterId]>>,
    slots: Arena<GenericParameterId, GenericParameter<AccessCodeId>>,
}

impl Parameters {
    pub(super) fn intern_types(&self, code: AccessCodeId, types: &mut TypeTransaction) {
        for &id in self.ids(code) {
            if self
                .get(id)
                .is_some_and(|slot| slot.domain() == GenericParameterDomain::Type)
            {
                types
                    .intern(TypeKind::GenericParameter(id))
                    .expect("reserved type parameter");
            }
        }
    }

    pub(super) fn ids(&self, code: AccessCodeId) -> &[GenericParameterId] {
        self.by_code.get(&code).map_or(&[], Box::as_ref)
    }

    pub(super) fn get(&self, id: GenericParameterId) -> Option<GenericParameter<AccessCodeId>> {
        self.slots.get(id).copied()
    }

    pub(super) fn named(
        &self,
        code: AccessCodeId,
        name: Symbol,
    ) -> Option<(GenericParameterId, GenericParameter<AccessCodeId>)> {
        named(self.ids(code), name, |id| self.get(id))
    }
}

fn named(
    ids: &[GenericParameterId],
    name: Symbol,
    mut get: impl FnMut(GenericParameterId) -> Option<GenericParameter<AccessCodeId>>,
) -> Option<(GenericParameterId, GenericParameter<AccessCodeId>)> {
    ids.iter().find_map(|&id| {
        let parameter = get(id)?;
        (parameter.name() == name).then_some((id, parameter))
    })
}
