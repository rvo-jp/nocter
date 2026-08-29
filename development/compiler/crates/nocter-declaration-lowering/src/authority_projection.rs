use std::collections::HashMap;

use nocter_compile_input::CompileUnitInput;
use nocter_frontend_bindings::FrontendBindings;
use nocter_source_index::SourceIndex;

use crate::{CurrentProjectionError, ReusableDeclarations};

/// Current source projection restricted to declaration-authority preparation.
///
/// It deliberately omits block imports and body spellings. The contained source identities may
/// explain a current authored rejection, but no generation-local identity is retained by a
/// successful reusable checking authority.
pub struct DeclarationAuthorityProjection {
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
}

impl DeclarationAuthorityProjection {
    #[must_use]
    pub fn into_parts(self) -> (FrontendBindings, SourceIndex) {
        (self.frontend_bindings, self.source_index)
    }
}

impl ReusableDeclarations {
    /// Materializes only the declaration projection required to build program-wide authorities.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when stable declaration locators cannot bind into the supplied
    /// current source domain.
    pub fn materialize_authority_projection(
        &self,
        input: &CompileUnitInput<'_>,
    ) -> Result<DeclarationAuthorityProjection, CurrentProjectionError> {
        let sources = crate::current_projection::canonical_sources(input);
        let (source_index, frontend_bindings) =
            self.projection_recipe()
                .materialize(input.sources(), &sources, &HashMap::new())?;
        Ok(DeclarationAuthorityProjection {
            frontend_bindings,
            source_index,
        })
    }
}
