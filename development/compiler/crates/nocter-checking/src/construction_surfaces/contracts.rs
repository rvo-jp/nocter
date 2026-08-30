use nocter_model::{
    ArgumentPackType, CallableId, DeclarationSiteId, FieldId, GenericParameterId, RequirementId,
    TypeId,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct StructuralFieldEntry {
    field: FieldId,
    site: DeclarationSiteId,
}

impl StructuralFieldEntry {
    pub(super) const fn new(field: FieldId, site: DeclarationSiteId) -> Self {
        Self { field, site }
    }

    pub(super) const fn field(self) -> FieldId {
        self.field
    }

    pub(super) const fn site(self) -> DeclarationSiteId {
        self.site
    }
}

/// A borrowed view of the one ordered structural-field authority.
#[derive(Clone, Copy)]
pub(crate) struct StructuralFields<'a> {
    entries: &'a [StructuralFieldEntry],
}

impl<'a> StructuralFields<'a> {
    pub(super) const fn new(entries: &'a [StructuralFieldEntry]) -> Self {
        Self { entries }
    }

    pub(crate) fn iter(self) -> impl ExactSizeIterator<Item = FieldId> + 'a {
        self.entries
            .iter()
            .copied()
            .map(StructuralFieldEntry::field)
    }

    pub(crate) fn contains(self, field: FieldId) -> bool {
        self.entries.iter().any(|entry| entry.field() == field)
    }
}

/// One literal constructor whose complete static declaration shape was validated during
/// preparation.
///
/// Expression checking may specialize this contract, but it never has to rediscover what the
/// declaration means by reading the declaration graph again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedLiteralConstructor {
    construction_target: TypeId,
    construction_parameters: Box<[GenericParameterId]>,
    callable: CallableId,
    site: DeclarationSiteId,
    pack_type: Option<ArgumentPackType>,
    result: TypeId,
    requirements: Box<[RequirementId]>,
}

impl CheckedLiteralConstructor {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        construction_target: TypeId,
        construction_parameters: impl Into<Box<[GenericParameterId]>>,
        callable: CallableId,
        site: DeclarationSiteId,
        pack_type: Option<ArgumentPackType>,
        result: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            construction_target,
            construction_parameters: construction_parameters.into(),
            callable,
            site,
            pack_type,
            result,
            requirements: requirements.into(),
        }
    }

    #[must_use]
    pub(crate) const fn construction_target(&self) -> TypeId {
        self.construction_target
    }

    #[must_use]
    pub(crate) const fn construction_parameters(&self) -> &[GenericParameterId] {
        &self.construction_parameters
    }

    #[must_use]
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub(crate) const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub(crate) const fn pack_type(&self) -> Option<ArgumentPackType> {
        self.pack_type
    }

    #[must_use]
    pub(crate) const fn result(&self) -> TypeId {
        self.result
    }

    #[must_use]
    pub(crate) const fn requirements(&self) -> &[RequirementId] {
        &self.requirements
    }
}
