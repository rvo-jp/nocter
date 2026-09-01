//! Typed integrity failures retained by semantic query products.

use std::fmt;

/// Compiler-domain failure produced while joining otherwise reusable semantic authority to one
/// exact current source generation.
///
/// These failures are neither authored rejection nor computation-kernel failure. Keeping their
/// original type prevents a broken projection or stage contract from becoming an unexplained
/// missing-authority result at the session boundary.
#[derive(Debug)]
pub enum SemanticQueryFailure {
    CompileInput(nocter_discovery::CompileInputError),
    CurrentProjection(nocter_declaration_lowering::CurrentProjectionError),
    ProgramPreparation(nocter_checking::PreparationError),
    BodyNameResolution(nocter_checking::ReusableProgramBodyNameError),
    BodyChecking(nocter_checking::ReusableProgramBodyCheckError),
    ProgramFinalization(nocter_checking::PreparationFailure),
    NameRejectionMaterialization(nocter_checking::PreparationFailure),
    MissingBodyIdentity {
        path: Box<str>,
        locator: nocter_syntax::DeclarationSyntaxLocator,
    },
    UnexpectedAcceptedNameCatalog,
    InvalidStageTransition(&'static str),
}

impl fmt::Display for SemanticQueryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompileInput(error) => error.fmt(formatter),
            Self::CurrentProjection(error) => error.fmt(formatter),
            Self::ProgramPreparation(error) => error.fmt(formatter),
            Self::BodyNameResolution(error) => error.fmt(formatter),
            Self::BodyChecking(error) => error.fmt(formatter),
            Self::ProgramFinalization(error) => {
                write!(
                    formatter,
                    "program finalization failed internally: {error:?}"
                )
            }
            Self::NameRejectionMaterialization(error) => write!(
                formatter,
                "name rejection could not be materialized: {error:?}"
            ),
            Self::MissingBodyIdentity { path, locator } => write!(
                formatter,
                "semantic query has no declared body identity for {path} at {locator:?}"
            ),
            Self::UnexpectedAcceptedNameCatalog => formatter.write_str(
                "a rejected body-name set unexpectedly materialized as an accepted catalog",
            ),
            Self::InvalidStageTransition(transition) => {
                write!(formatter, "invalid semantic query transition: {transition}")
            }
        }
    }
}

impl std::error::Error for SemanticQueryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompileInput(error) => Some(error),
            Self::CurrentProjection(error) => Some(error),
            Self::ProgramPreparation(error) => Some(error),
            Self::BodyNameResolution(error) => Some(error),
            Self::BodyChecking(error) => Some(error),
            Self::ProgramFinalization(_)
            | Self::NameRejectionMaterialization(_)
            | Self::MissingBodyIdentity { .. }
            | Self::UnexpectedAcceptedNameCatalog
            | Self::InvalidStageTransition(_) => None,
        }
    }
}

impl From<nocter_discovery::CompileInputError> for SemanticQueryFailure {
    fn from(error: nocter_discovery::CompileInputError) -> Self {
        Self::CompileInput(error)
    }
}

impl From<nocter_declaration_lowering::CurrentProjectionError> for SemanticQueryFailure {
    fn from(error: nocter_declaration_lowering::CurrentProjectionError) -> Self {
        Self::CurrentProjection(error)
    }
}

impl From<nocter_checking::PreparationError> for SemanticQueryFailure {
    fn from(error: nocter_checking::PreparationError) -> Self {
        Self::ProgramPreparation(error)
    }
}

impl From<nocter_checking::ReusableProgramBodyNameError> for SemanticQueryFailure {
    fn from(error: nocter_checking::ReusableProgramBodyNameError) -> Self {
        Self::BodyNameResolution(error)
    }
}

impl From<nocter_checking::ReusableProgramBodyCheckError> for SemanticQueryFailure {
    fn from(error: nocter_checking::ReusableProgramBodyCheckError) -> Self {
        Self::BodyChecking(error)
    }
}
