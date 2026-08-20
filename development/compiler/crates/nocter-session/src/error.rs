use std::fmt;

/// A failure at one owned compilation-session boundary.
#[derive(Debug)]
pub enum CompileSessionError {
    CompileInput(nocter_discovery::CompileInputError),
    Declaration(nocter_declaration_lowering::DeclarationLoweringError),
    Preparation(nocter_checking::PreparationError),
    Checking(nocter_checking::BodyCheckError),
    MissingToolchainProfile,
    MissingStandardPackage,
    Primitive(nocter_target_program::PrimitiveResolutionError),
    TargetUnavailable(nocter_target_program::TargetUnavailable),
    Target(nocter_target_program::TargetProgramError),
}

impl fmt::Display for CompileSessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompileInput(error) => error.fmt(formatter),
            Self::Declaration(error) => error.fmt(formatter),
            Self::Preparation(error) => error.fmt(formatter),
            Self::Checking(error) => error.fmt(formatter),
            Self::MissingToolchainProfile => {
                formatter.write_str("compile input has no toolchain profile")
            }
            Self::MissingStandardPackage => {
                formatter.write_str("checked program has no selected standard package")
            }
            Self::Primitive(error) => error.fmt(formatter),
            Self::TargetUnavailable(error) => error.fmt(formatter),
            Self::Target(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileSessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CompileInput(error) => Some(error),
            Self::Declaration(error) => Some(error),
            Self::Preparation(error) => Some(error),
            Self::Checking(error) => Some(error),
            Self::Primitive(error) => Some(error),
            Self::TargetUnavailable(error) => Some(error),
            Self::Target(error) => Some(error),
            Self::MissingToolchainProfile | Self::MissingStandardPackage => None,
        }
    }
}

impl From<nocter_discovery::CompileInputError> for CompileSessionError {
    fn from(error: nocter_discovery::CompileInputError) -> Self {
        Self::CompileInput(error)
    }
}

impl From<nocter_declaration_lowering::DeclarationLoweringError> for CompileSessionError {
    fn from(error: nocter_declaration_lowering::DeclarationLoweringError) -> Self {
        Self::Declaration(error)
    }
}

impl From<nocter_checking::PreparationError> for CompileSessionError {
    fn from(error: nocter_checking::PreparationError) -> Self {
        Self::Preparation(error)
    }
}

impl From<nocter_checking::BodyCheckError> for CompileSessionError {
    fn from(error: nocter_checking::BodyCheckError) -> Self {
        Self::Checking(error)
    }
}

impl From<nocter_target_program::PrimitiveResolutionError> for CompileSessionError {
    fn from(error: nocter_target_program::PrimitiveResolutionError) -> Self {
        Self::Primitive(error)
    }
}

impl From<nocter_target_program::TargetUnavailable> for CompileSessionError {
    fn from(error: nocter_target_program::TargetUnavailable) -> Self {
        Self::TargetUnavailable(error)
    }
}

impl From<nocter_target_program::TargetProgramError> for CompileSessionError {
    fn from(error: nocter_target_program::TargetProgramError) -> Self {
        Self::Target(error)
    }
}
