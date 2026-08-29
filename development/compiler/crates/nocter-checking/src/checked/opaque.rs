use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{Arena, ArenaBuilder, BodyNodeId, OpaqueTypeId, TypeId};

/// One checked conversion from a concrete witness representation to its opaque public result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedOpaqueWitness {
    value: BodyNodeId,
    definition: OpaqueTypeId,
    witness: TypeId,
}

impl CheckedOpaqueWitness {
    pub(super) fn rebind(
        &mut self,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<(), super::CheckedSemanticRebindError> {
        self.witness = semantics.ty(self.witness)?;
        Ok(())
    }
    pub(crate) const fn new(value: BodyNodeId, definition: OpaqueTypeId, witness: TypeId) -> Self {
        Self {
            value,
            definition,
            witness,
        }
    }

    #[must_use]
    pub const fn value(self) -> BodyNodeId {
        self.value
    }

    #[must_use]
    pub const fn definition(self) -> OpaqueTypeId {
        self.definition
    }

    #[must_use]
    pub const fn witness(self) -> TypeId {
        self.witness
    }
}

/// The sole concrete witness pattern selected for every declaration-scoped opaque result.
#[derive(Clone, Debug)]
pub struct OpaqueWitnessTable {
    witnesses: Arena<OpaqueTypeId, TypeId>,
}

impl OpaqueWitnessTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        selections: impl IntoIterator<Item = (OpaqueTypeId, TypeId)>,
    ) -> Result<Self, OpaqueWitnessTableBuildError> {
        let mut selected = BTreeMap::new();
        for (definition, witness) in selections {
            if graph
                .declarations()
                .opaque_types()
                .get(definition)
                .is_none()
            {
                return Err(OpaqueWitnessTableBuildError::UnknownDefinition(definition));
            }
            if selected.insert(definition, witness).is_some() {
                return Err(OpaqueWitnessTableBuildError::DuplicateDefinition(
                    definition,
                ));
            }
        }
        let mut witnesses = ArenaBuilder::new();
        for (definition, _) in graph.declarations().opaque_types().iter() {
            let witness = selected
                .remove(&definition)
                .ok_or(OpaqueWitnessTableBuildError::MissingWitness(definition))?;
            let actual = witnesses.insert(witness);
            if actual != definition {
                return Err(OpaqueWitnessTableBuildError::NonCanonicalDefinition(
                    definition,
                ));
            }
        }
        if !selected.is_empty() {
            return Err(OpaqueWitnessTableBuildError::ResidualSelection);
        }
        Ok(Self {
            witnesses: witnesses.finish(),
        })
    }

    #[must_use]
    pub fn get(&self, definition: OpaqueTypeId) -> Option<TypeId> {
        self.witnesses.get(definition).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpaqueWitnessTableBuildError {
    UnknownDefinition(OpaqueTypeId),
    DuplicateDefinition(OpaqueTypeId),
    MissingWitness(OpaqueTypeId),
    NonCanonicalDefinition(OpaqueTypeId),
    ResidualSelection,
}

impl fmt::Display for OpaqueWitnessTableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "opaque-witness table invariant failed: {self:?}")
    }
}

impl std::error::Error for OpaqueWitnessTableBuildError {}
