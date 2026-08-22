use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{ModuleId, NominalTypeId, Symbol, VariantId};

use crate::{
    CheckedProgram, ConstructionSurfaceSelectionError, ConstructionSurfaceTable,
    PreparedSemanticProgram, SelectedConstructionEntry,
};

/// One variant available in an enum-pattern position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnumPatternCompletionCandidate {
    variant: VariantId,
    name: Symbol,
}

impl EnumPatternCompletionCandidate {
    #[must_use]
    pub const fn variant(self) -> VariantId {
        self.variant
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }
}

/// Failure to derive enum-pattern completion from immutable compiler authorities.
#[derive(Debug)]
pub enum EnumPatternCompletionError {
    Surface(ConstructionSurfaceSelectionError),
    MissingVariant(VariantId),
    InvalidVariant(VariantId),
}

impl fmt::Display for EnumPatternCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Surface(error) => error.fmt(formatter),
            Self::MissingVariant(variant) => {
                write!(formatter, "enum-pattern variant {variant:?} is absent")
            }
            Self::InvalidVariant(variant) => write!(
                formatter,
                "enum-pattern variant {variant:?} belongs to another type"
            ),
        }
    }
}

impl std::error::Error for EnumPatternCompletionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Surface(error) => Some(error),
            Self::MissingVariant(_) | Self::InvalidVariant(_) => None,
        }
    }
}

impl From<ConstructionSurfaceSelectionError> for EnumPatternCompletionError {
    fn from(error: ConstructionSurfaceSelectionError) -> Self {
        Self::Surface(error)
    }
}

impl CheckedProgram {
    /// Enumerates only variants valid in a pattern for the resolved enum family.
    ///
    /// # Errors
    ///
    /// Returns an error when checked variant identities disagree with the construction surface.
    pub fn enum_pattern_completions(
        &self,
        definition: NominalTypeId,
        module: ModuleId,
    ) -> Result<Box<[EnumPatternCompletionCandidate]>, EnumPatternCompletionError> {
        select_enum_pattern_completions(
            self.graph(),
            self.construction_surfaces(),
            definition,
            module,
        )
    }
}

impl PreparedSemanticProgram {
    /// Enumerates pattern variants from the completed pre-body semantic authority.
    ///
    /// # Errors
    ///
    /// Returns an error when retained variant identities disagree with the construction surface.
    pub fn enum_pattern_completions(
        &self,
        definition: NominalTypeId,
        module: ModuleId,
    ) -> Result<Box<[EnumPatternCompletionCandidate]>, EnumPatternCompletionError> {
        select_enum_pattern_completions(
            self.graph(),
            self.construction_surfaces(),
            definition,
            module,
        )
    }
}

fn select_enum_pattern_completions(
    graph: &DeclarationGraph,
    surfaces: &ConstructionSurfaceTable,
    definition: NominalTypeId,
    module: ModuleId,
) -> Result<Box<[EnumPatternCompletionCandidate]>, EnumPatternCompletionError> {
    surfaces
        .accessible_surface(graph, definition, module)?
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            SelectedConstructionEntry::Variant(variant) => Some(*variant),
            SelectedConstructionEntry::Structural | SelectedConstructionEntry::Callable(_) => None,
        })
        .map(|variant| {
            let declaration = graph
                .declarations()
                .variants()
                .get(variant)
                .ok_or(EnumPatternCompletionError::MissingVariant(variant))?;
            if declaration.owner() != definition {
                return Err(EnumPatternCompletionError::InvalidVariant(variant));
            }
            Ok(EnumPatternCompletionCandidate {
                variant,
                name: declaration.name(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}
