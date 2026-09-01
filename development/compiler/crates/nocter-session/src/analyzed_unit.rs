use std::sync::Arc;

use nocter_discovery::DiscoveredUnit;

use crate::analysis::{
    CompileTargetFailure, IncompleteSyntaxAnalysis, failure_from_finalization,
    failure_from_incomplete_semantics, failure_from_name_resolution, failure_from_preparation,
    target_from_finalized_program,
};
use crate::{CompileSessionFailure, CompiledTarget, SemanticEvidenceBundle, SemanticEvidenceView};

/// The compiler authority retained for one source-complete or explicitly recovered analysis run.
#[derive(Debug)]
enum AnalyzedUnitState {
    SyntaxFailed {
        recovery_failure: Option<CompileSessionFailure>,
        semantic: Option<Box<SemanticEvidenceBundle>>,
    },
    CompilationFailed {
        failure: CompileSessionFailure,
        semantic: Option<Box<SemanticEvidenceBundle>>,
    },
    Complete(Box<CompiledTarget>),
}

/// The semantic completion state of one analyzed discovery snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyzedUnitStatus {
    SyntaxFailed,
    CompilationFailed,
    Complete,
}

/// One closed session failure paired with the exact source and diagnostic snapshot selected by
/// the unit-analysis query.
#[derive(Debug)]
pub struct AnalyzedCompilationFailure {
    failure: CompileSessionFailure,
    sources: nocter_source::SourceMap,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
}

impl AnalyzedCompilationFailure {
    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        CompileSessionFailure,
        nocter_source::SourceMap,
        Box<[nocter_diagnostics::SourceDiagnostic]>,
    ) {
        (self.failure, self.sources, self.diagnostics)
    }
}

impl std::fmt::Display for AnalyzedCompilationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl std::error::Error for AnalyzedCompilationFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

/// One discovery snapshot paired inseparably with the exact session outcome derived from it.
///
/// Consumers cannot combine semantic evidence from one source graph with another graph. The
/// semantic query product has already selected stage order and recovery; session validates that
/// source domain once and translates the closed branch without reopening the compiler pipeline.
#[derive(Debug)]
pub struct AnalyzedUnit {
    unit: Arc<DiscoveredUnit>,
    diagnostics: Box<[nocter_diagnostics::SourceDiagnostic]>,
    state: AnalyzedUnitState,
}

impl AnalyzedUnit {
    #[must_use]
    pub const fn status(&self) -> AnalyzedUnitStatus {
        match self.state {
            AnalyzedUnitState::SyntaxFailed { .. } => AnalyzedUnitStatus::SyntaxFailed,
            AnalyzedUnitState::CompilationFailed { .. } => AnalyzedUnitStatus::CompilationFailed,
            AnalyzedUnitState::Complete(_) => AnalyzedUnitStatus::Complete,
        }
    }

    #[must_use]
    pub fn unit(&self) -> &DiscoveredUnit {
        &self.unit
    }

    #[must_use]
    pub const fn diagnostics(&self) -> &[nocter_diagnostics::SourceDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn semantic_evidence(&self) -> Option<SemanticEvidenceView<'_>> {
        match &self.state {
            AnalyzedUnitState::SyntaxFailed { semantic, .. }
            | AnalyzedUnitState::CompilationFailed { semantic, .. } => {
                semantic.as_deref().map(SemanticEvidenceBundle::view)
            }
            AnalyzedUnitState::Complete(target) => Some(target.semantic_evidence()),
        }
    }

    /// Consumes this session result through the target boundary used by command compilation.
    ///
    /// # Errors
    ///
    /// Returns the exact syntax or semantic failure selected by the query-owned analysis branch.
    pub fn into_compilation_result(
        self,
    ) -> Result<CompiledTarget, Box<AnalyzedCompilationFailure>> {
        let Self {
            unit,
            diagnostics,
            state,
        } = self;
        match state {
            AnalyzedUnitState::Complete(target) => Ok(*target),
            AnalyzedUnitState::SyntaxFailed {
                recovery_failure, ..
            } => {
                let continuations = recovery_failure
                    .map(CompileSessionFailure::into_errors)
                    .unwrap_or_default();
                Err(Box::new(AnalyzedCompilationFailure {
                    failure: CompileSessionFailure::new(
                        crate::CompileSessionError::SyntaxErrorsPresent,
                        continuations,
                    ),
                    sources: unit.sources().clone(),
                    diagnostics,
                }))
            }
            AnalyzedUnitState::CompilationFailed { failure, .. } => {
                Err(Box::new(AnalyzedCompilationFailure {
                    failure,
                    sources: unit.sources().clone(),
                    diagnostics,
                }))
            }
        }
    }
}

/// Consumes the sole query-owned complete-or-incomplete semantic outcome.
///
/// # Errors
///
/// Returns an integrity error when the query graph did not publish required semantic authority.
pub fn analyze_unit_from_query(
    product: &nocter_semantic_product::UnitAnalysisProduct,
) -> Result<AnalyzedUnit, SemanticAnalysisDomainError> {
    let unit = Arc::clone(product.unit());
    match product.outcome() {
        nocter_semantic_product::UnitAnalysisOutcome::Complete(complete) => {
            analyzed_complete_unit(unit, complete)
        }
        nocter_semantic_product::UnitAnalysisOutcome::Incomplete(incomplete) => {
            Ok(analyzed_incomplete_unit(
                unit,
                crate::analysis::incomplete_syntax_analysis(incomplete),
            ))
        }
        nocter_semantic_product::UnitAnalysisOutcome::Failed(failure) => Err(
            SemanticAnalysisDomainError::QueryFailure(Arc::clone(failure)),
        ),
    }
}

fn analyzed_complete_unit(
    unit: Arc<DiscoveredUnit>,
    product: &nocter_semantic_product::ProgramAnalysisProduct,
) -> Result<AnalyzedUnit, SemanticAnalysisDomainError> {
    let analyzed = match product.outcome() {
        nocter_semantic_product::ProgramAnalysisOutcome::Checked(finalized) => {
            match target_from_finalized_program(&unit, finalized) {
                Ok(target) => AnalyzedUnit {
                    unit,
                    diagnostics: Box::new([]),
                    state: AnalyzedUnitState::Complete(Box::new(target)),
                },
                Err(failure) => analyzed_compilation_failure(unit, *failure),
            }
        }
        nocter_semantic_product::ProgramAnalysisOutcome::NamesRejected(failed) => {
            analyzed_compilation_failure(unit, failure_from_name_resolution(failed.failure()))
        }
        nocter_semantic_product::ProgramAnalysisOutcome::BodiesRejected(failed) => {
            analyzed_compilation_failure(unit, failure_from_finalization(failed.failure()))
        }
        nocter_semantic_product::ProgramAnalysisOutcome::PreparationRejected(rejected) => {
            analyzed_compilation_failure(unit, failure_from_preparation(rejected.rejection()))
        }
        nocter_semantic_product::ProgramAnalysisOutcome::DeclarationsRejected(failed) => {
            analyzed_compilation_failure(unit, failure_from_incomplete_semantics(failed.failure()))
        }
        nocter_semantic_product::ProgramAnalysisOutcome::Failed(failure) => {
            return Err(SemanticAnalysisDomainError::QueryFailure(Arc::clone(
                failure,
            )));
        }
    };
    Ok(analyzed)
}

fn analyzed_compilation_failure(
    unit: Arc<DiscoveredUnit>,
    failure: CompileTargetFailure,
) -> AnalyzedUnit {
    let (failure, semantic, diagnostics) = failure.into_analysis_parts();
    AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed {
            failure,
            semantic: semantic.map(Box::new),
        },
    }
}

fn analyzed_incomplete_unit(
    unit: Arc<DiscoveredUnit>,
    analysis: IncompleteSyntaxAnalysis,
) -> AnalyzedUnit {
    let (recovery_failure, semantic, semantic_diagnostics) = analysis.into_analysis_parts();
    let mut diagnostics = unit.syntax_diagnostics().into_vec();
    extend_unique_diagnostics(&mut diagnostics, &semantic_diagnostics);
    AnalyzedUnit {
        unit,
        diagnostics: diagnostics.into_boxed_slice(),
        state: AnalyzedUnitState::SyntaxFailed {
            recovery_failure,
            semantic: semantic.map(Box::new),
        },
    }
}

#[derive(Clone, Debug)]
pub enum SemanticAnalysisDomainError {
    QueryFailure(Arc<nocter_semantic_product::SemanticQueryFailure>),
}

impl std::fmt::Display for SemanticAnalysisDomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueryFailure(failure) => failure.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticAnalysisDomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::QueryFailure(failure) => Some(failure.as_ref()),
        }
    }
}

fn extend_unique_diagnostics(
    diagnostics: &mut Vec<nocter_diagnostics::SourceDiagnostic>,
    candidates: &[nocter_diagnostics::SourceDiagnostic],
) {
    for diagnostic in candidates {
        if !diagnostics.contains(diagnostic) {
            diagnostics.push(diagnostic.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_diagnostics::{DiagnosticCode, SourceDiagnostic};
    use nocter_source::{ByteOffset, SourceMap, SourceName, TextRange};

    use super::extend_unique_diagnostics;

    #[test]
    fn diagnostic_composition_deduplicates_identity_without_inventing_range_causality() {
        let mut sources = SourceMap::new();
        let source = sources
            .add_bytes(SourceName::new("diagnostics.nct"), b"diagnostic")
            .unwrap();
        let span = sources
            .get(source)
            .unwrap()
            .span(TextRange::new(ByteOffset::new(4), ByteOffset::new(7)));
        let syntax = SourceDiagnostic::new(DiagnosticCode::E0120, "syntax", span, [], None::<&str>);
        let semantic =
            SourceDiagnostic::new(DiagnosticCode::E0120, "syntax", span, [], None::<&str>);
        let distinct =
            SourceDiagnostic::new(DiagnosticCode::E0120, "syntax", span, [], Some("detail"));
        let mut diagnostics = vec![syntax.clone()];

        extend_unique_diagnostics(&mut diagnostics, &[semantic, distinct.clone()]);

        assert_eq!(diagnostics, vec![syntax, distinct]);
    }
}
