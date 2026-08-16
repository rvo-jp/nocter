use std::collections::BTreeMap;

use nocter_declarations::{CallableKind, CallableOwner, DeclarationGraph};
use nocter_model::{
    CallableId, ConstructionId, ModuleId, NominalTypeId, Symbol, TypeId, TypeStore,
};

use crate::type_relations::InherentTypeFamily;

/// Canonical construction-surface lookup prepared once for the complete program.
///
/// Declaration validation has already guaranteed one construction declaration per target family.
/// Keeping the index in the checked program gives body checking and later editor queries the same
/// authority instead of making each consumer scan declarations independently.
#[derive(Debug)]
pub struct ConstructionSurfaceTable {
    by_family: BTreeMap<InherentTypeFamily, ConstructionId>,
}

impl ConstructionSurfaceTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &TypeStore,
    ) -> Result<Self, ConstructionSurfaceBuildError> {
        let mut by_family = BTreeMap::new();
        for (construction, declaration) in graph.declarations().constructions().iter() {
            let family = InherentTypeFamily::of(types, declaration.target()).ok_or(
                ConstructionSurfaceBuildError::InvalidTarget(declaration.target()),
            )?;
            if by_family.insert(family, construction).is_some() {
                return Err(ConstructionSurfaceBuildError::DuplicateTarget(
                    declaration.target(),
                ));
            }
        }
        Ok(Self { by_family })
    }

    #[must_use]
    pub fn for_nominal(&self, nominal: NominalTypeId) -> Option<ConstructionId> {
        self.by_family
            .get(&InherentTypeFamily::Nominal(nominal))
            .copied()
    }

    pub(crate) fn for_type(&self, types: &TypeStore, ty: TypeId) -> Option<ConstructionId> {
        InherentTypeFamily::of(types, ty).and_then(|family| self.by_family.get(&family).copied())
    }

    /// Selects one accessible named construction function from an already resolved owner family.
    ///
    /// Member spelling, ownership, callable kind, and visibility are resolved here so body
    /// checking and editor consumers cannot grow separate construction-member lookup rules.
    ///
    /// # Errors
    ///
    /// Returns an internal selection error if the validated declaration graph and surface index
    /// disagree or if multiple accessible members have the same semantic name.
    pub fn named_function(
        &self,
        graph: &DeclarationGraph,
        construction: ConstructionId,
        name: Symbol,
        from: ModuleId,
    ) -> Result<Option<CallableId>, ConstructionSurfaceSelectionError> {
        let declaration = graph
            .declarations()
            .constructions()
            .get(construction)
            .ok_or(ConstructionSurfaceSelectionError::MissingConstruction(
                construction,
            ))?;
        let mut selected = None;
        for member in declaration.members().iter().copied() {
            let callable = graph
                .declarations()
                .callables()
                .get(member)
                .ok_or(ConstructionSurfaceSelectionError::MissingCallable(member))?;
            if callable.owner() != CallableOwner::Construction(construction) {
                return Err(ConstructionSurfaceSelectionError::InvalidMember(member));
            }
            if callable.kind() != CallableKind::ConstructionFunction
                || callable.name() != Some(name)
            {
                continue;
            }
            let site = graph
                .declaration_sites()
                .get(callable.site())
                .copied()
                .ok_or(ConstructionSurfaceSelectionError::MissingCallableSite(
                    member,
                ))?;
            if !graph.is_visible_from(site.visibility(), from, site.module()) {
                continue;
            }
            if selected.replace(member).is_some() {
                return Err(ConstructionSurfaceSelectionError::AmbiguousMember(
                    construction,
                    name,
                ));
            }
        }
        Ok(selected)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_family.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_family.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionSurfaceBuildError {
    InvalidTarget(TypeId),
    DuplicateTarget(TypeId),
}

impl std::fmt::Display for ConstructionSurfaceBuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTarget(target) => {
                write!(formatter, "invalid construction target {target:?}")
            }
            Self::DuplicateTarget(target) => {
                write!(formatter, "duplicate construction target {target:?}")
            }
        }
    }
}

impl std::error::Error for ConstructionSurfaceBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstructionSurfaceSelectionError {
    MissingConstruction(ConstructionId),
    MissingCallable(CallableId),
    MissingCallableSite(CallableId),
    InvalidMember(CallableId),
    AmbiguousMember(ConstructionId, Symbol),
}

impl std::fmt::Display for ConstructionSurfaceSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid construction surface selection: {self:?}"
        )
    }
}

impl std::error::Error for ConstructionSurfaceSelectionError {}
