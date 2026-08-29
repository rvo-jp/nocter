use nocter_compile_input::CompileUnitInput;
use nocter_frontend_bindings::FrontendBindings;
use nocter_model::BodyId;

use crate::{
    BodySourceError, PreparedSemanticProgram, ReusableBodyNames, ReusableBodyResolutionError,
    ReusablePreparedProgram, catalog_body_source, resolve_reusable_body_names,
};

/// Exact-current program view shared by every body-name query in one revision.
#[derive(Debug)]
pub struct ProgramBodyNameContext {
    current: PreparedSemanticProgram,
}

impl ProgramBodyNameContext {
    #[must_use]
    pub fn new<S>(
        program: &ReusablePreparedProgram,
        checking_spellings: impl IntoIterator<Item = S>,
        bindings: &FrontendBindings,
    ) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            current: program.open_current(checking_spellings, bindings.source_access().clone()),
        }
    }

    /// Resolves one body from this context into a source-neutral result.
    ///
    /// # Errors
    ///
    /// Returns an inconsistent current body source or lexical resolution/projection failure.
    pub fn resolve(
        &self,
        input: &CompileUnitInput<'_>,
        bindings: &FrontendBindings,
        body: BodyId,
    ) -> Result<ReusableBodyNames, ReusableProgramBodyNameError> {
        let source = catalog_body_source(input, self.current.graph(), bindings, body)
            .map_err(ReusableProgramBodyNameError::BodySource)?;
        resolve_reusable_body_names(input, self.current.graph(), bindings, source)
            .map_err(ReusableProgramBodyNameError::Resolution)
    }
}

/// Resolves one body against reusable program authorities and publishes only source-neutral name
/// evidence.
///
/// # Errors
///
/// Returns a target/source-domain mismatch or the exact body resolution/projection failure.
pub fn resolve_reusable_program_body_names<S>(
    input: &CompileUnitInput<'_>,
    program: &ReusablePreparedProgram,
    checking_spellings: impl IntoIterator<Item = S>,
    bindings: &FrontendBindings,
    body: BodyId,
) -> Result<ReusableBodyNames, ReusableProgramBodyNameError>
where
    S: AsRef<str>,
{
    if input.target() != program.graph().target() {
        return Err(ReusableProgramBodyNameError::TargetMismatch);
    }
    ProgramBodyNameContext::new(program, checking_spellings, bindings)
        .resolve(input, bindings, body)
}

#[derive(Debug)]
pub enum ReusableProgramBodyNameError {
    TargetMismatch,
    BodySource(BodySourceError),
    Resolution(ReusableBodyResolutionError),
}

impl std::fmt::Display for ReusableProgramBodyNameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TargetMismatch => formatter.write_str("body query target does not match program"),
            Self::BodySource(error) => error.fmt(formatter),
            Self::Resolution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReusableProgramBodyNameError {}
