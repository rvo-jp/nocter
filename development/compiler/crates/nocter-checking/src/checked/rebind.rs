use nocter_model::{ClosureId, TypeId};

use crate::{
    BodyClosureRecipe, BodyClosureRecipeError, BodyTypeRecipe, BodyTypeRecipeError,
    ReplayedBodyClosures, ReplayedBodyTypes,
};

/// Exact mapping from one reusable body branch into the current canonical program branch.
pub(crate) struct CheckedSemanticRebinder<'a> {
    source_types: &'a BodyTypeRecipe,
    target_types: &'a ReplayedBodyTypes,
    source_closures: &'a BodyClosureRecipe,
    target_closures: &'a ReplayedBodyClosures,
}

impl<'a> CheckedSemanticRebinder<'a> {
    pub(crate) const fn new(
        source_types: &'a BodyTypeRecipe,
        target_types: &'a ReplayedBodyTypes,
        source_closures: &'a BodyClosureRecipe,
        target_closures: &'a ReplayedBodyClosures,
    ) -> Self {
        Self {
            source_types,
            target_types,
            source_closures,
            target_closures,
        }
    }

    pub(crate) fn ty(&self, source: TypeId) -> Result<TypeId, CheckedSemanticRebindError> {
        Ok(self
            .target_types
            .resolve(self.source_types.reference(source)?)?)
    }

    pub(crate) fn closure(
        &self,
        source: ClosureId,
    ) -> Result<ClosureId, CheckedSemanticRebindError> {
        Ok(self
            .target_closures
            .resolve(self.source_closures.reference(source)?)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CheckedSemanticRebindError {
    Type(BodyTypeRecipeError),
    Closure(BodyClosureRecipeError),
}

impl From<BodyTypeRecipeError> for CheckedSemanticRebindError {
    fn from(error: BodyTypeRecipeError) -> Self {
        Self::Type(error)
    }
}

impl From<BodyClosureRecipeError> for CheckedSemanticRebindError {
    fn from(error: BodyClosureRecipeError) -> Self {
        Self::Closure(error)
    }
}

impl std::fmt::Display for CheckedSemanticRebindError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid checked semantic rebind: {self:?}")
    }
}

impl std::error::Error for CheckedSemanticRebindError {}
