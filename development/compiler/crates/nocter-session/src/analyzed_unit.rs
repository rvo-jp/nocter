use std::sync::Arc;

use nocter_discovery::DiscoveredUnit;

use crate::analysis::{
    analyze_target_from_declaration_failure, analyze_target_from_declarations,
    analyze_target_from_finalization_failure, analyze_target_from_finalized_program,
    analyze_target_from_name_resolution_failure, analyze_target_from_preparation_rejection,
    analyze_target_from_prepared_declarations,
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

/// Consumes a complete query-owned checked program without invoking semantic compiler stages.
///
/// # Errors
///
/// Returns an integrity error when the product belongs to a different exact source domain.
pub fn analyze_unit_from_finalized_program(
    unit: Arc<DiscoveredUnit>,
    finalized: &nocter_semantic_computation::FinalizedProgram,
) -> Result<AnalyzedUnit, SemanticRejectionDomainError> {
    validate_rejection_domain(&unit, finalized.unit())?;
    if unit.has_syntax_errors() {
        return Ok(analyze_unit(unit));
    }
    Ok(
        match analyze_target_from_finalized_program(&unit, finalized) {
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
        },
    )
}

/// Consumes a query-owned whole-program checking failure without replaying or finalizing bodies.
///
/// # Errors
///
/// Returns an integrity error when the failure belongs to a different exact source domain.
pub fn analyze_unit_from_finalization_failure(
    unit: Arc<DiscoveredUnit>,
    failed: &nocter_semantic_computation::FailedProgramFinalization,
) -> Result<AnalyzedUnit, SemanticRejectionDomainError> {
    validate_rejection_domain(&unit, failed.unit())?;
    if unit.has_syntax_errors() {
        return Ok(analyze_unit(unit));
    }
    let failure = analyze_target_from_finalization_failure(failed.failure());
    let (semantic, diagnostics) = (*failure).into_analysis_parts();
    Ok(AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
    })
}

/// Consumes a query-owned lexical rejection without invoking name resolution or body checking.
///
/// # Errors
///
/// Returns an integrity error when the failure belongs to a different exact source domain.
pub fn analyze_unit_from_name_resolution_failure(
    unit: Arc<DiscoveredUnit>,
    failed: &nocter_semantic_computation::FailedProgramNameResolution,
) -> Result<AnalyzedUnit, SemanticRejectionDomainError> {
    validate_rejection_domain(&unit, failed.unit())?;
    if unit.has_syntax_errors() {
        return Ok(analyze_unit(unit));
    }
    let failure = analyze_target_from_name_resolution_failure(failed.failure());
    let (semantic, diagnostics) = (*failure).into_analysis_parts();
    Ok(AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
    })
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
/// session remains the sole owner of compiler stage order and recovery selection.
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
        let (semantic, semantic_diagnostics) = analyze_incomplete_syntax(&unit).map_or(
            (
                None,
                Box::<[nocter_diagnostics::SourceDiagnostic]>::default(),
            ),
            crate::IncompleteSyntaxAnalysis::into_analysis_parts,
        );
        let mut diagnostics = unit.syntax_diagnostics().into_vec();
        extend_unique_diagnostics(&mut diagnostics, &semantic_diagnostics);
        return AnalyzedUnit {
            unit,
            diagnostics: diagnostics.into_boxed_slice(),
            state: AnalyzedUnitState::SyntaxFailed(semantic.map(Box::new)),
        };
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

/// Consumes one current discovery snapshot using declarations computed by the semantic query.
#[must_use]
pub fn analyze_unit_from_declarations(
    unit: Arc<DiscoveredUnit>,
    declarations: &nocter_declaration_lowering::ReusableDeclarations,
) -> AnalyzedUnit {
    if unit.has_syntax_errors() {
        return analyze_unit(unit);
    }
    match analyze_target_from_declarations(&unit, declarations) {
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

/// Consumes one current discovery snapshot using query-owned declaration and preparation products.
#[must_use]
pub fn analyze_unit_from_prepared_declarations(
    unit: Arc<DiscoveredUnit>,
    declarations: &nocter_declaration_lowering::ReusableDeclarations,
    prepared: &nocter_checking::ReusablePreparedProgram,
) -> AnalyzedUnit {
    if unit.has_syntax_errors() {
        return analyze_unit(unit);
    }
    match analyze_target_from_prepared_declarations(&unit, declarations, prepared) {
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

/// Consumes one current discovery snapshot and its query-owned declaration rejection.
///
/// # Errors
///
/// Returns an integrity error when the rejection was produced from a different semantic topology
/// or exact current-source identity layout.
pub fn analyze_unit_from_declaration_failure(
    unit: Arc<DiscoveredUnit>,
    rejection_unit: &DiscoveredUnit,
    failure: &nocter_declaration_lowering::DeclarationLoweringFailure,
) -> Result<AnalyzedUnit, SemanticRejectionDomainError> {
    validate_rejection_domain(&unit, rejection_unit)?;
    if unit.has_syntax_errors() {
        return Ok(analyze_unit(unit));
    }
    let failure = analyze_target_from_declaration_failure(&unit, failure);
    let (semantic, diagnostics) = (*failure).into_analysis_parts();
    Ok(AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
    })
}

/// Consumes one current discovery snapshot and its query-owned preparation rejection.
///
/// # Errors
///
/// Returns an integrity error when the rejection belongs to a different exact source domain.
pub fn analyze_unit_from_preparation_rejection(
    unit: Arc<DiscoveredUnit>,
    rejection_unit: &DiscoveredUnit,
    rejection: &nocter_checking::QueriedProgramPreparationRejection,
) -> Result<AnalyzedUnit, SemanticRejectionDomainError> {
    validate_rejection_domain(&unit, rejection_unit)?;
    if unit.has_syntax_errors() {
        return Ok(analyze_unit(unit));
    }
    let failure = analyze_target_from_preparation_rejection(rejection);
    let (semantic, diagnostics) = (*failure).into_analysis_parts();
    Ok(AnalyzedUnit {
        unit,
        diagnostics,
        state: AnalyzedUnitState::CompilationFailed(semantic.map(Box::new)),
    })
}

fn validate_rejection_domain(
    current: &DiscoveredUnit,
    rejection: &DiscoveredUnit,
) -> Result<(), SemanticRejectionDomainError> {
    if std::ptr::eq(current, rejection) {
        return Ok(());
    }
    let current_topology = current.semantic_topology_surface()?;
    let rejection_topology = rejection.semantic_topology_surface()?;
    let current_sources = current.current_source_surface()?;
    let rejection_sources = rejection.current_source_surface()?;
    if current_topology != rejection_topology || current_sources != rejection_sources {
        return Err(SemanticRejectionDomainError::Mismatch);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticRejectionDomainError {
    SemanticTopology(nocter_discovery::SemanticTopologyError),
    CurrentSource(nocter_discovery::CurrentSourceSurfaceError),
    Mismatch,
}

impl std::fmt::Display for SemanticRejectionDomainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemanticTopology(error) => error.fmt(formatter),
            Self::CurrentSource(error) => error.fmt(formatter),
            Self::Mismatch => formatter
                .write_str("semantic rejection and current analysis use different source domains"),
        }
    }
}

impl std::error::Error for SemanticRejectionDomainError {}

impl From<nocter_discovery::SemanticTopologyError> for SemanticRejectionDomainError {
    fn from(error: nocter_discovery::SemanticTopologyError) -> Self {
        Self::SemanticTopology(error)
    }
}

impl From<nocter_discovery::CurrentSourceSurfaceError> for SemanticRejectionDomainError {
    fn from(error: nocter_discovery::CurrentSourceSurfaceError) -> Self {
        Self::CurrentSource(error)
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
