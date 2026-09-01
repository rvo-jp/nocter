use std::sync::Arc;

use nocter_compile_input::CompileUnitInput;
use nocter_declaration_lowering::{
    CurrentProjectionError, DeclarationBodyAnalysisInput, LoweredDeclarations, ReusableDeclarations,
};

use crate::preparation::{ReusablePreparedProgram, ReusableProgramPreparationQueryOutcome};
use crate::{
    BodyAnalysisRecovery, BodyCheckFailure, CheckedProgramOutput, PreparationError,
    PreparationFailure, ProgramBodyCheckingContext, QueriedProgramPreparationRejection,
};

/// Source-neutral checking authorities paired with the declaration recipe that alone may reopen
/// them against a current source generation.
#[derive(Debug)]
pub struct ReusableCheckingQuery {
    declarations: Arc<ReusableDeclarations>,
    prepared: ReusablePreparedProgram,
}

/// Complete preparation result for a reusable semantic query.
#[derive(Debug)]
pub enum ReusableCheckingQueryOutcome {
    Prepared(Box<ReusableCheckingQuery>),
    Rejected(Box<QueriedProgramPreparationRejection>),
}

/// Failure to construct the reusable checking owner from one current source generation.
#[derive(Debug)]
pub enum ReusableCheckingQueryError {
    CurrentProjection(CurrentProjectionError),
    Preparation(PreparationError),
}

impl ReusableCheckingQuery {
    /// Builds program-wide checking authorities while retaining the only declaration recipe that
    /// may later materialize their exact-current body context.
    ///
    /// # Errors
    ///
    /// Returns an exact-current projection failure or a program-wide preparation failure.
    pub fn prepare(
        input: &CompileUnitInput<'_>,
        declarations: Arc<ReusableDeclarations>,
    ) -> Result<ReusableCheckingQueryOutcome, ReusableCheckingQueryError> {
        let projection = declarations
            .materialize_authority_projection(input)
            .map_err(ReusableCheckingQueryError::CurrentProjection)?;
        let (bindings, source_index) = projection.into_parts();
        match crate::preparation::prepare_reusable_program_for_query(
            input,
            declarations.checking_branch(),
            &bindings,
            source_index,
        )
        .map_err(ReusableCheckingQueryError::Preparation)?
        {
            ReusableProgramPreparationQueryOutcome::Prepared(prepared) => {
                Ok(ReusableCheckingQueryOutcome::Prepared(Box::new(Self {
                    declarations,
                    prepared: *prepared,
                })))
            }
            ReusableProgramPreparationQueryOutcome::Rejected(rejection) => {
                Ok(ReusableCheckingQueryOutcome::Rejected(rejection))
            }
        }
    }

    /// Opens the exact-current body-query context through the paired declaration recipe.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when the current source domain cannot materialize the recipe.
    pub fn open_current(
        &self,
        input: &CompileUnitInput<'_>,
    ) -> Result<ProgramBodyCheckingContext, CurrentProjectionError> {
        let projection = self.declarations.materialize_projection(input)?;
        Ok(ProgramBodyCheckingContext::new(&self.prepared, projection))
    }
}

/// Failure of the complete declaration-to-checked recovery transition.
#[derive(Debug)]
pub enum LoweredProgramCheckFailure {
    Preparation(PreparationFailure),
    Checking(BodyCheckFailure),
}

/// Checks one complete lowered declaration product without exposing its paired checking parts.
///
/// # Errors
///
/// Returns the preparation or typed-body failure at the rejecting boundary.
pub fn check_lowered_program_recovering(
    input: &CompileUnitInput<'_>,
    lowered: LoweredDeclarations,
) -> Result<CheckedProgramOutput, LoweredProgramCheckFailure> {
    let (program, bindings, source_index) = lowered.into_checking_parts();
    let prepared = crate::preparation::prepare_program_checking_recovering(
        input,
        program,
        &bindings,
        source_index,
    )
    .map_err(LoweredProgramCheckFailure::Preparation)?;
    crate::body_check::check_prepared_program_recovering(input, prepared)
        .map_err(LoweredProgramCheckFailure::Checking)
}

/// Failure of the complete rejected-declaration body-analysis transition.
#[derive(Debug)]
pub enum DeclarationBodyAnalysisFailure {
    Preparation(PreparationFailure),
    Checking(BodyCheckFailure),
}

/// Analyzes bodies from one admitted declaration-recovery product without exposing its parts.
///
/// # Errors
///
/// Returns the preparation or typed-body failure together with its explicit recovery capability.
pub fn analyze_declaration_bodies(
    input: &CompileUnitInput<'_>,
    admitted: DeclarationBodyAnalysisInput,
) -> Result<BodyAnalysisRecovery, DeclarationBodyAnalysisFailure> {
    let (program, bindings, source_index) = admitted.into_parts();
    let prepared = crate::preparation::prepare_analysis_program_checking_recovering(
        input,
        program,
        &bindings,
        source_index,
    )
    .map_err(DeclarationBodyAnalysisFailure::Preparation)?;
    crate::body_check::analyze_prepared_program_bodies(input, prepared)
        .map_err(DeclarationBodyAnalysisFailure::Checking)
}
