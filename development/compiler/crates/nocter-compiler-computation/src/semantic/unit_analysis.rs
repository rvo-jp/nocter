//! Sole complete-or-incomplete unit query.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{CurrentSourceScopeInput, SemanticScopeKey};

struct UnitAnalysisQuery;

/// Sole reusable complete-or-incomplete semantic branch for one source-content revision.
#[derive(Debug)]
pub enum UnitAnalysisOutcome {
    Complete(Arc<super::ProgramAnalysisProduct>),
    Incomplete(Arc<super::IncompleteSemanticAnalysis>),
    Failed(Arc<super::SemanticQueryFailure>),
}

/// One exact discovery snapshot paired inseparably with its reusable semantic branch.
///
/// This envelope is deliberately constructed outside the query cache. Editor document versions
/// and other source-overlay metadata may advance while the source bytes and semantic result remain
/// unchanged; reusing the semantic branch must never retain the previous revision's source
/// envelope.
#[derive(Debug)]
pub struct UnitAnalysisProduct {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    semantic: Arc<CachedUnitAnalysis>,
}

#[derive(Debug)]
pub(super) struct CachedUnitAnalysis {
    outcome: UnitAnalysisOutcome,
    fingerprint: Fingerprint,
}

impl UnitAnalysisProduct {
    pub(super) const fn new(
        unit: Arc<nocter_discovery::DiscoveredUnit>,
        semantic: Arc<CachedUnitAnalysis>,
    ) -> Self {
        Self { unit, semantic }
    }

    #[must_use]
    pub const fn unit(&self) -> &Arc<nocter_discovery::DiscoveredUnit> {
        &self.unit
    }

    #[must_use]
    pub fn outcome(&self) -> &UnitAnalysisOutcome {
        &self.semantic.outcome
    }
}

impl QueryValue for CachedUnitAnalysis {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for UnitAnalysisQuery {
    type Key = SemanticScopeKey;
    type Value = CachedUnitAnalysis;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let outcome = if current.unit.has_syntax_errors() {
            let incomplete = super::incomplete_analysis(database, key.clone())?;
            match incomplete.outcome() {
                super::incomplete_analysis::IncompleteAnalysisOutcome::Analyzed(analysis) => {
                    UnitAnalysisOutcome::Incomplete(Arc::clone(analysis))
                }
                super::incomplete_analysis::IncompleteAnalysisOutcome::Failed(failure) => {
                    UnitAnalysisOutcome::Failed(Arc::clone(failure))
                }
            }
        } else {
            let complete = super::analyzed_program(database, key.clone())?;
            UnitAnalysisOutcome::Complete(complete)
        };
        Ok(CachedUnitAnalysis {
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

/// Demands the sole semantic outcome for one published source scope.
///
/// # Errors
///
/// Returns computation-kernel failures. Authored rejection and typed query-integrity failure
/// remain inside the selected branch.
pub(super) fn analyzed_unit(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<CachedUnitAnalysis>, ComputationError> {
    database.query::<UnitAnalysisQuery>(key)
}

#[must_use]
pub(super) fn unit_analysis_execution_count(database: &Database) -> u64 {
    database.execution_count::<UnitAnalysisQuery>()
}

#[must_use]
pub(super) fn unit_analysis_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<UnitAnalysisQuery>()
}
