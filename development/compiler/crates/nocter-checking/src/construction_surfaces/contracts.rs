use nocter_model::{CallableId, DeclarationSiteId, GenericParameterId, RequirementId, TypeId};

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
    parameter_type: TypeId,
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
        parameter_type: TypeId,
        result: TypeId,
        requirements: impl Into<Box<[RequirementId]>>,
    ) -> Self {
        Self {
            construction_target,
            construction_parameters: construction_parameters.into(),
            callable,
            site,
            parameter_type,
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
    pub(crate) const fn parameter_type(&self) -> TypeId {
        self.parameter_type
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
