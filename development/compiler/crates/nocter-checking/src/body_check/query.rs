use nocter_compile_input::CompileUnitInput;
use nocter_frontend_bindings::FrontendBindings;
use nocter_source_index::SourceIndex;

use super::checker::{BodyChecker, BodyUnitInput};
use super::context::BodyProgramFacts;
use super::reusable_body::{ReusableCheckedBody, capture_checked_body};
use super::semantic_transaction::BodySemanticAuthority;
use crate::checked::ClosureAuthority;
use crate::preparation::ReusablePreparedProgram;
use crate::{
    BodySourceError, ReusableBodyNameQueryOutcome, ReusableBodyNames, ReusableBodyNamesError,
    ReusableBodyResolutionError,
};

/// Exact-current projection required to produce independent source-neutral typed-body results.
///
/// The context owns only program authorities and current lookup projections. A body query must
/// supply both its exact current source and its reusable lexical result; sibling body state is not
/// an input and cannot affect semantic identity allocation.
#[derive(Debug)]
pub struct ProgramBodyCheckingContext<'syntax> {
    current: crate::PreparedSemanticProgram,
    frontend_bindings: FrontendBindings,
    source_index: SourceIndex,
    body_sources: crate::BodySourceCatalog<'syntax>,
}

#[derive(Debug)]
pub enum ReusableBodyQueryOutcome {
    Checked(ReusableCheckedBody),
    Rejected(QueriedBodyRejection),
}

#[derive(Debug)]
pub struct QueriedBodyRejection {
    body: nocter_model::BodyId,
    error: crate::BodyCheckError,
    rejection: crate::BodyRejection,
}

impl QueriedBodyRejection {
    #[must_use]
    pub const fn body(&self) -> nocter_model::BodyId {
        self.body
    }

    pub(crate) fn clone_parts(&self) -> Option<(crate::BodyCheckError, crate::BodyRejection)> {
        Some((self.error.clone_authored()?, self.rejection.clone()))
    }
}

impl<'syntax> ProgramBodyCheckingContext<'syntax> {
    /// Materializes one complete query-owned lexical catalog against this current checking context.
    ///
    /// # Errors
    ///
    /// Returns the authored lexical rejection with canonical recovery, or an internal preparation
    /// failure when the catalog cannot be joined to the current context.
    pub fn prepare_names(
        &self,
        names: &[&ReusableBodyNames],
        rejections: &[&crate::QueriedBodyNameRejection],
    ) -> Result<crate::PreparedChecking<'syntax>, crate::PreparationFailure> {
        crate::preparation::prepare_program_checking_from_current_queried_names(
            self.current.clone(),
            &self.frontend_bindings,
            self.source_index.clone(),
            self.body_sources.clone(),
            names,
            rejections,
        )
    }

    pub(crate) fn new(
        program: &ReusablePreparedProgram,
        projection: nocter_declaration_lowering::CurrentDeclarationProjection,
        input: &CompileUnitInput<'syntax>,
    ) -> Result<Self, BodySourceError> {
        let (frontend_bindings, source_index, checking_symbols) = projection.into_parts();
        let current = program.open_current(
            checking_symbols.spellings(),
            frontend_bindings.source_access().clone(),
        );
        let body_sources = crate::catalog_body_sources(input, current.graph(), &frontend_bindings)?;
        Ok(Self {
            current,
            frontend_bindings,
            source_index,
            body_sources,
        })
    }

    /// Resolves one body against the same current semantic projection used by typed checking.
    ///
    /// # Errors
    ///
    /// Returns an inconsistent body source or lexical resolution/projection failure.
    pub fn resolve_names(
        &self,
        input: &CompileUnitInput<'_>,
        body: nocter_model::BodyId,
    ) -> Result<ReusableBodyNameQueryOutcome, ReusableProgramBodyNameError> {
        let source = self
            .body_sources
            .project(body)
            .ok_or(BodySourceError::MissingBodyProjection(body))
            .map_err(ReusableProgramBodyNameError::BodySource)?;
        crate::names::resolve_reusable_body_names_for_query(
            input,
            self.current.graph(),
            &self.frontend_bindings,
            source.source(),
            source.syntax(),
        )
        .map_err(ReusableProgramBodyNameError::Resolution)
    }

    /// Checks exactly one body and returns a result containing no current source identities.
    ///
    /// # Errors
    ///
    /// Returns the exact source/name projection failure or typed-body diagnostic for this body.
    pub fn check(
        &self,
        input: &CompileUnitInput<'_>,
        names: &ReusableBodyNames,
    ) -> Result<ReusableBodyQueryOutcome, ReusableProgramBodyCheckError> {
        let body = names.body();
        let source = self
            .body_sources
            .project(body)
            .ok_or(BodySourceError::MissingBodyProjection(body))
            .map_err(ReusableProgramBodyCheckError::BodySource)?;
        let (names, _) = names
            .materialize(self.current.graph(), source.source(), source.syntax())
            .map_err(ReusableProgramBodyCheckError::Names)?;
        let facts = BodyProgramFacts::new(
            self.current.environment(),
            self.frontend_bindings.source_namespaces(),
            self.current.source_access(),
            self.source_index.diagnostic_origins(),
        );
        let program_semantics = self.current.semantics().clone();
        let mut body_semantics =
            BodySemanticAuthority::new(program_semantics.clone(), ClosureAuthority::new());
        let mut transaction = body_semantics.transaction();
        let closure_ids = {
            let mut access = transaction.access();
            super::pipeline::reserve_body_closures(access.closures(), source.source())
        };
        let unit = BodyUnitInput {
            source: source.source(),
            names: &names,
            closure_ids,
        };
        let attempt = BodyChecker::new(input, facts, transaction.access(), unit)
            .map_err(ReusableProgramBodyCheckError::Checking)?
            .check();
        match attempt {
            Ok(output) => {
                body_semantics = transaction
                    .commit(&body_semantics)
                    .map_err(|_| crate::BodyCheckInternalError::BodySemanticCommit)
                    .map_err(crate::BodyCheckError::from)
                    .map_err(ReusableProgramBodyCheckError::Checking)?;
                capture_checked_body(
                    &program_semantics,
                    &body_semantics,
                    source.source(),
                    source.syntax(),
                    output,
                )
                .map(ReusableBodyQueryOutcome::Checked)
                .map_err(crate::BodyCheckError::from)
                .map_err(ReusableProgramBodyCheckError::Checking)
            }
            Err(failure) => {
                let recovery_semantics = transaction.freeze_recovery();
                let interruption =
                    super::pipeline::retain_interruption_evidence(&failure, recovery_semantics)
                        .map_err(crate::BodyCheckError::from)
                        .map_err(ReusableProgramBodyCheckError::Checking)?;
                let (rejection, error) =
                    super::pipeline::classify_body_rejection(body, failure, interruption)
                        .map_err(ReusableProgramBodyCheckError::Checking)?;
                Ok(ReusableBodyQueryOutcome::Rejected(QueriedBodyRejection {
                    body,
                    error,
                    rejection,
                }))
            }
        }
    }

    /// Replays the complete queried body set into one exact-current owned program materialization.
    ///
    /// # Errors
    ///
    /// Returns a preparation integrity failure when the supplied lexical or typed body set cannot
    /// be joined to this context's current declaration and source projections.
    pub fn materialize(
        &self,
        body_names: &[&ReusableBodyNames],
        body_rejections: &[&crate::QueriedBodyNameRejection],
        bodies: &[&ReusableCheckedBody],
        rejections: &[&QueriedBodyRejection],
    ) -> Result<crate::QueriedProgramMaterializationOutcome, crate::PreparationFailure> {
        let prepared = self.prepare_names(body_names, body_rejections)?;
        Ok(super::materialize_prepared_program_from_queried_bodies(
            prepared, bodies, rejections,
        ))
    }
}

#[derive(Debug)]
pub enum ReusableProgramBodyNameError {
    BodySource(BodySourceError),
    Resolution(ReusableBodyResolutionError),
}

impl std::fmt::Display for ReusableProgramBodyNameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodySource(error) => error.fmt(formatter),
            Self::Resolution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReusableProgramBodyNameError {}

#[derive(Debug)]
pub enum ReusableProgramBodyCheckError {
    BodySource(BodySourceError),
    Names(ReusableBodyNamesError),
    Checking(crate::BodyCheckError),
}

impl std::fmt::Display for ReusableProgramBodyCheckError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BodySource(error) => error.fmt(formatter),
            Self::Names(error) => error.fmt(formatter),
            Self::Checking(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ReusableProgramBodyCheckError {}
