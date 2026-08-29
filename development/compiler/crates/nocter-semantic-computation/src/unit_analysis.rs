use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use crate::{CurrentSourceScopeInput, SemanticScopeKey};

struct UnitAnalysisQuery;

/// Required top-level authority absent from an otherwise valid computation revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnitAnalysisUnavailable {
    Complete,
    Incomplete,
}

impl std::fmt::Display for UnitAnalysisUnavailable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Complete => "source-complete program analysis",
            Self::Incomplete => "incomplete-syntax analysis",
        })
    }
}

/// Sole complete-or-incomplete semantic branch for one exact source revision.
#[derive(Debug)]
pub enum UnitAnalysisOutcome {
    Complete(Arc<crate::ProgramAnalysisProduct>),
    Incomplete(crate::IncompleteSemanticAnalysis),
    Unavailable(UnitAnalysisUnavailable),
}

/// One exact discovery snapshot paired inseparably with its closed semantic branch.
#[derive(Debug)]
pub struct UnitAnalysisProduct {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    outcome: UnitAnalysisOutcome,
    fingerprint: Fingerprint,
}

impl UnitAnalysisProduct {
    #[must_use]
    pub const fn unit(&self) -> &Arc<nocter_discovery::DiscoveredUnit> {
        &self.unit
    }

    #[must_use]
    pub const fn outcome(&self) -> &UnitAnalysisOutcome {
        &self.outcome
    }
}

impl QueryValue for UnitAnalysisProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for UnitAnalysisQuery {
    type Key = SemanticScopeKey;
    type Value = UnitAnalysisProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let outcome = if current.unit.has_syntax_errors() {
            let incomplete = crate::incomplete_analysis(database, key.clone())?;
            incomplete.analysis().cloned().map_or(
                UnitAnalysisOutcome::Unavailable(UnitAnalysisUnavailable::Incomplete),
                UnitAnalysisOutcome::Incomplete,
            )
        } else {
            let complete = crate::analyzed_program(database, key.clone())?;
            UnitAnalysisOutcome::Complete(complete)
        };
        Ok(UnitAnalysisProduct {
            unit: Arc::clone(&current.unit),
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

/// Demands the sole semantic outcome for one published source scope.
///
/// # Errors
///
/// Returns computation-kernel failures. Authored rejection remains inside the selected branch.
pub fn analyzed_unit(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<UnitAnalysisProduct>, ComputationError> {
    database.query::<UnitAnalysisQuery>(key)
}

#[must_use]
pub fn unit_analysis_execution_count(database: &Database) -> u64 {
    database.execution_count::<UnitAnalysisQuery>()
}

#[must_use]
pub fn unit_analysis_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<UnitAnalysisQuery>()
}
