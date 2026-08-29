use std::sync::Arc;

use nocter_discovery::DiscoveredUnit;

use crate::analysis::{
    analyze_target_from_finalization_failure, analyze_target_from_finalized_program,
    analyze_target_from_name_resolution_failure, analyze_target_from_preparation_rejection,
    analyze_target_from_semantic_failure,
};
use crate::{
    CompiledTarget, SemanticEvidenceBundle, SemanticEvidenceView, analyze_incomplete_syntax,
    analyze_target,
};

/// The compiler authority retained for one source-complete or explicitly recovered analysis run.
#[derive(Debug)]
enum AnalyzedUnitState {
    SyntaxFailed(Option<Box<SemanticEvidenceBundle>>),
    CompilationFailed(Option<Box<SemanticEvidenceBundle>>),
    Complete(Box<CompiledTarget>),
}

/// The semantic completion state of one analyzed discovery snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyzedUnitStatus {
    SyntaxFailed,
    CompilationFailed,
    Complete,
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
            AnalyzedUnitState::SyntaxFailed(_) => AnalyzedUnitStatus::SyntaxFailed,
            AnalyzedUnitState::CompilationFailed(_) => AnalyzedUnitStatus::CompilationFailed,
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
            AnalyzedUnitState::SyntaxFailed(evidence)
            | AnalyzedUnitState::CompilationFailed(evidence) => {
                evidence.as_deref().map(SemanticEvidenceBundle::view)
            }
            AnalyzedUnitState::Complete(target) => Some(target.semantic_evidence()),
        }
    }
}

/// Consumes one immutable discovery snapshot through the editor semantic boundary.
///
/// Syntax-invalid input runs only the explicit incomplete-syntax admission. Syntax-clean input
/// runs the production target analysis once. The returned value owns both the source graph and its
/// exact outcome, so no later layer can substitute either half.
#[must_use]
pub fn analyze_unit(unit: Arc<DiscoveredUnit>) -> AnalyzedUnit {
    if unit.has_syntax_errors() {
        let analysis =
            analyze_incomplete_syntax(&unit).unwrap_or_else(crate::IncompleteSyntaxAnalysis::empty);
        return analyzed_incomplete_unit(unit, analysis);
    }

    match analyze_target(&unit) {
        Ok(target) => AnalyzedUnit {
            unit,
            diagnostics: Box::new([]),
            state: AnalyzedUnitState::Complete(Box::new(target)),
        },
        Err(failure) => {
            let (semantic, diagnostics) = (*failure).into_analysis_parts();
            AnalyzedUnit {
                unit,
                diagnostics,
                state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
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
    product: &nocter_semantic_computation::UnitAnalysisProduct,
) -> Result<AnalyzedUnit, SemanticAnalysisDomainError> {
    let unit = Arc::clone(product.unit());
    match product.outcome() {
        nocter_semantic_computation::UnitAnalysisOutcome::Complete(complete) => {
            analyzed_complete_unit(unit, complete)
        }
        nocter_semantic_computation::UnitAnalysisOutcome::Incomplete(incomplete) => {
            Ok(analyzed_incomplete_unit(
                unit,
                crate::analysis::incomplete_syntax_analysis(incomplete),
            ))
        }
        nocter_semantic_computation::UnitAnalysisOutcome::Unavailable(authority) => Err(
            SemanticAnalysisDomainError::UnavailableUnitAnalysis(*authority),
        ),
    }
}

fn analyzed_complete_unit(
    unit: Arc<DiscoveredUnit>,
    product: &nocter_semantic_computation::ProgramAnalysisProduct,
) -> Result<AnalyzedUnit, SemanticAnalysisDomainError> {
    let analyzed = match product.outcome() {
        nocter_semantic_computation::ProgramAnalysisOutcome::Checked(finalized) => {
            match analyze_target_from_finalized_program(&unit, finalized) {
                Ok(target) => AnalyzedUnit {
                    unit,
                    diagnostics: Box::new([]),
                    state: AnalyzedUnitState::Complete(Box::new(target)),
                },
                Err(failure) => analyzed_compilation_failure(unit, *failure),
            }
        }
        nocter_semantic_computation::ProgramAnalysisOutcome::NamesRejected(failed) => {
            analyzed_compilation_failure(
                unit,
                *analyze_target_from_name_resolution_failure(failed.failure()),
            )
        }
        nocter_semantic_computation::ProgramAnalysisOutcome::BodiesRejected(failed) => {
            analyzed_compilation_failure(
                unit,
                *analyze_target_from_finalization_failure(failed.failure()),
            )
        }
        nocter_semantic_computation::ProgramAnalysisOutcome::PreparationRejected(rejected) => {
            analyzed_compilation_failure(
                unit,
                *analyze_target_from_preparation_rejection(rejected.rejection()),
            )
        }
        nocter_semantic_computation::ProgramAnalysisOutcome::DeclarationsRejected(failed) => {
            analyzed_compilation_failure(
                unit,
                *analyze_target_from_semantic_failure(failed.failure()),
            )
        }
        nocter_semantic_computation::ProgramAnalysisOutcome::Unavailable(authority) => {
            return Err(SemanticAnalysisDomainError::UnavailableProgramAnalysis(
                *authority,
            ));
        }
    };
    Ok(analyzed)
}

fn analyzed_compilation_failure(
    unit: Arc<DiscoveredUnit>,
    failure: crate::CompileTargetFailure,
) -> AnalyzedUnit {
    let (semantic, diagnostics) = failure.into_analysis_parts();
    AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
    }
}

fn analyzed_incomplete_unit(
    unit: Arc<DiscoveredUnit>,
    analysis: crate::IncompleteSyntaxAnalysis,
) -> AnalyzedUnit {
    let (semantic, semantic_diagnostics) = analysis.into_analysis_parts();
    let mut diagnostics = unit.syntax_diagnostics().into_vec();
    extend_unique_diagnostics(&mut diagnostics, &semantic_diagnostics);
    AnalyzedUnit {
        unit,
        diagnostics: diagnostics.into_boxed_slice(),
        state: AnalyzedUnitState::SyntaxFailed(semantic.map(Box::new)),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticAnalysisDomainError {
    UnavailableUnitAnalysis(nocter_semantic_computation::UnitAnalysisUnavailable),
    UnavailableProgramAnalysis(nocter_semantic_computation::ProgramAnalysisUnavailable),
}

impl std::fmt::Display for SemanticAnalysisDomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnavailableUnitAnalysis(authority) => {
                write!(formatter, "unit analysis is missing {authority} authority")
            }
            Self::UnavailableProgramAnalysis(authority) => write!(
                formatter,
                "source-complete semantic analysis is missing {authority} authority"
            ),
        }
    }
}

impl std::error::Error for SemanticAnalysisDomainError {}

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
    use nocter_diagnostics::SourceDiagnostic;
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
        let syntax = SourceDiagnostic::new("E0120", "syntax", span, [], None::<&str>);
        let semantic = SourceDiagnostic::new("E0120", "syntax", span, [], None::<&str>);
        let distinct = SourceDiagnostic::new("E0120", "syntax", span, [], Some("detail"));
        let mut diagnostics = vec![syntax.clone()];

        extend_unique_diagnostics(&mut diagnostics, &[semantic, distinct.clone()]);

        assert_eq!(diagnostics, vec![syntax, distinct]);
    }
}
