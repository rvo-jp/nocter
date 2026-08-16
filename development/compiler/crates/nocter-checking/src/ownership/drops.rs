use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{DropId, NominalTypeId, TypeId, TypeKind, TypeStore};

/// Canonical association between nominal families and their type-owned drop bodies.
#[derive(Debug, Default)]
pub struct DropTable {
    families: BTreeMap<NominalTypeId, DropId>,
}

impl DropTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<Self, DropTableError> {
        let mut families = BTreeMap::new();
        for (drop, declaration) in graph.declarations().drops().iter() {
            let definition = match types.get(declaration.target()) {
                Some(TypeKind::Nominal { definition, .. }) => *definition,
                Some(_) => return Err(DropTableError::InvalidTarget(declaration.target())),
                None => return Err(DropTableError::UnknownType(declaration.target())),
            };
            if families.insert(definition, drop).is_some() {
                return Err(DropTableError::DuplicateFamily(definition));
            }
        }
        Ok(Self { families })
    }

    #[must_use]
    pub fn get(&self, family: NominalTypeId) -> Option<DropId> {
        self.families.get(&family).copied()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DropTableError {
    UnknownType(TypeId),
    InvalidTarget(TypeId),
    DuplicateFamily(NominalTypeId),
}

impl fmt::Display for DropTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "drop-table invariant failed: {self:?}")
    }
}

impl std::error::Error for DropTableError {}
