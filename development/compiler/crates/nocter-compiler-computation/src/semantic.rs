//! Demand-owned semantic queries behind the compiler-computation entry.

mod body_context;
mod body_names;
mod incomplete_analysis;
mod program_analysis;
mod program_finalization;
mod program_preparation;
mod typed_bodies;
mod unit_analysis;

use body_names::{
    BodyNameQueryOutcome, BodyNameSet, SemanticBodyKey, resolve_body_name, resolved_body_names,
};
pub use incomplete_analysis::{
    IncompleteSemanticAnalysis, IncompleteSemanticError, IncompleteSemanticEvidence,
    IncompleteSemanticFailure,
};
use incomplete_analysis::{analyze_declaration_failure, incomplete_analysis};
use program_analysis::analyzed_program;
pub use program_analysis::{
    ProgramAnalysisOutcome, ProgramAnalysisProduct, ProgramAnalysisUnavailable,
};
pub use program_finalization::FinalizedProgram;
use program_finalization::{
    FailedProgramFinalization, FailedProgramNameResolution, ProgramFinalizationOutcome,
    finalized_program,
};
use program_preparation::{
    ProgramPreparationOutcome, RejectedProgramPreparation, prepared_program,
};
use typed_bodies::{TypedBodySet, typed_bodies};
pub use unit_analysis::{UnitAnalysisOutcome, UnitAnalysisProduct, UnitAnalysisUnavailable};

use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Input, InputRetention, InputRevision,
    Query, QueryValue,
};
use nocter_declaration_lowering::{
    DeclarationLoweringFailure, ReusableDeclarations, lower_reusable_declarations,
};
use nocter_discovery::DiscoveredUnit;

/// Stable identity of one selected semantic compile scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SemanticScopeKey {
    stable: Box<[u8]>,
}

impl SemanticScopeKey {
    #[must_use]
    fn for_unit(unit: &DiscoveredUnit) -> Self {
        let mut stable = Vec::new();
        encode(unit.target().name().as_bytes(), &mut stable);
        let mut roots = unit.root_packages().iter().collect::<Vec<_>>();
        roots.sort_unstable();
        for root in roots {
            encode(root.as_str().as_bytes(), &mut stable);
        }
        Self {
            stable: stable.into_boxed_slice(),
        }
    }
}

impl ComputationKey for SemanticScopeKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.stable.clone()
    }
}

/// Both invalidation views of one exact discovery snapshot.
struct ScopeInputPublication {
    unit: Arc<DiscoveredUnit>,
    declaration_fingerprint: Fingerprint,
    current_source_fingerprint: Fingerprint,
}

impl ScopeInputPublication {
    /// Builds both invalidation views from one exact discovery snapshot.
    ///
    /// # Errors
    ///
    /// Returns a discovery-integrity failure rather than publishing mismatched topology or source
    /// storage.
    fn for_unit(
        unit: Arc<DiscoveredUnit>,
        module_surface_fingerprint: Fingerprint,
    ) -> Result<(SemanticScopeKey, Self), SemanticInputError> {
        let key = SemanticScopeKey::for_unit(&unit);
        let topology = unit.semantic_topology_surface()?;
        let current = unit.current_source_surface()?;
        let mut declaration = topology.canonical_bytes().to_vec();
        declaration.extend_from_slice(&module_surface_fingerprint.digest());
        let declaration_fingerprint = Fingerprint::from_bytes(&declaration);
        let mut current_bytes = declaration_fingerprint.digest().to_vec();
        current_bytes.extend_from_slice(current.canonical_bytes());
        let current_source_fingerprint = Fingerprint::from_bytes(&current_bytes);
        Ok((
            key,
            Self::new(unit, declaration_fingerprint, current_source_fingerprint),
        ))
    }

    #[must_use]
    const fn new(
        unit: Arc<DiscoveredUnit>,
        declaration_fingerprint: Fingerprint,
        current_source_fingerprint: Fingerprint,
    ) -> Self {
        Self {
            unit,
            declaration_fingerprint,
            current_source_fingerprint,
        }
    }

    fn publish(self, revision: &mut InputRevision<'_>, key: &SemanticScopeKey) {
        revision.set::<DeclarationScopeInput>(
            key,
            ScopeInputValue {
                unit: Arc::clone(&self.unit),
                fingerprint: self.declaration_fingerprint,
            },
        );
        revision.set::<CurrentSourceScopeInput>(
            key,
            ScopeInputValue {
                unit: self.unit,
                fingerprint: self.current_source_fingerprint,
            },
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticInputError {
    SemanticTopology(nocter_discovery::SemanticTopologyError),
    CurrentSource(nocter_discovery::CurrentSourceSurfaceError),
}

impl std::fmt::Display for SemanticInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemanticTopology(error) => error.fmt(formatter),
            Self::CurrentSource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticInputError {}

impl From<nocter_discovery::SemanticTopologyError> for SemanticInputError {
    fn from(error: nocter_discovery::SemanticTopologyError) -> Self {
        Self::SemanticTopology(error)
    }
}

impl From<nocter_discovery::CurrentSourceSurfaceError> for SemanticInputError {
    fn from(error: nocter_discovery::CurrentSourceSurfaceError) -> Self {
        Self::CurrentSource(error)
    }
}

struct DeclarationScopeInput;
struct CurrentSourceScopeInput;
pub(super) struct BodySourceInput;

impl Input for DeclarationScopeInput {
    type Key = SemanticScopeKey;
    type Value = ScopeInputValue;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

impl Input for CurrentSourceScopeInput {
    type Key = SemanticScopeKey;
    type Value = ScopeInputValue;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

impl Input for BodySourceInput {
    type Key = BodySourceKey;
    type Value = BodySourceValue;

    const RETENTION: InputRetention = InputRetention::RevisionDerived;
}

/// Stable physical identity of one executable body beneath a declaration surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BodySourceKey {
    path: Box<str>,
    locator: nocter_syntax::DeclarationSyntaxLocator,
    stable: Box<[u8]>,
}

impl BodySourceKey {
    fn new(path: &str, locator: nocter_syntax::DeclarationSyntaxLocator) -> Self {
        let mut stable = Vec::new();
        encode(path.as_bytes(), &mut stable);
        match locator {
            nocter_syntax::DeclarationSyntaxLocator::Node(index) => {
                stable.push(0);
                stable.extend_from_slice(&index.to_be_bytes());
            }
            nocter_syntax::DeclarationSyntaxLocator::Token(index) => {
                stable.push(1);
                stable.extend_from_slice(&index.to_be_bytes());
            }
        }
        Self {
            path: path.into(),
            locator,
            stable: stable.into_boxed_slice(),
        }
    }
}

impl ComputationKey for BodySourceKey {
    fn stable_bytes(&self) -> Box<[u8]> {
        self.stable.clone()
    }
}

pub(super) struct BodySourceValue {
    fingerprint: Fingerprint,
}

impl QueryValue for BodySourceValue {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

/// One exact per-body input staged with its containing semantic scope revision.
pub(super) struct BodySourcePublication {
    key: BodySourceKey,
    value: BodySourceValue,
}

impl BodySourcePublication {
    #[must_use]
    pub(super) fn new(path: &str, body: &nocter_syntax::BodySyntaxSurface) -> Self {
        Self {
            key: BodySourceKey::new(path, body.locator()),
            value: BodySourceValue {
                fingerprint: Fingerprint::from_bytes(body.canonical_bytes()),
            },
        }
    }

    fn publish(self, revision: &mut InputRevision<'_>) {
        revision.set::<BodySourceInput>(&self.key, self.value);
    }
}

struct ScopeInputValue {
    unit: Arc<DiscoveredUnit>,
    fingerprint: Fingerprint,
}

impl QueryValue for ScopeInputValue {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

struct DeclarationQuery;

/// Source-neutral accepted declarations or an explicitly current-source-bound rejection.
#[derive(Debug)]
pub enum DeclarationQueryOutcome {
    Accepted(Arc<ReusableDeclarations>),
    Rejected(RejectedDeclarations),
    Unavailable,
}

/// One declaration rejection stored only inside an exact-current query product.
#[derive(Debug)]
pub struct RejectedDeclarations {
    failure: Arc<DeclarationLoweringFailure>,
}

impl RejectedDeclarations {
    #[must_use]
    pub fn failure(&self) -> &DeclarationLoweringFailure {
        &self.failure
    }
}

#[derive(Debug)]
pub struct DeclarationQueryProduct {
    outcome: DeclarationQueryOutcome,
    fingerprint: Fingerprint,
}

impl DeclarationQueryProduct {
    #[must_use]
    pub const fn outcome(&self) -> &DeclarationQueryOutcome {
        &self.outcome
    }

    #[must_use]
    pub(crate) const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl QueryValue for DeclarationQueryProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for DeclarationQuery {
    type Key = SemanticScopeKey;
    type Value = DeclarationQueryProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let semantic = database.input::<DeclarationScopeInput>(key)?;
        let failure = match semantic.unit.compile_input() {
            Ok(input) => match lower_reusable_declarations(&input) {
                Ok(lowered) => {
                    return Ok(DeclarationQueryProduct {
                        outcome: DeclarationQueryOutcome::Accepted(Arc::new(lowered)),
                        fingerprint: semantic.fingerprint,
                    });
                }
                Err(failure) => Some(failure),
            },
            Err(_) => None,
        };

        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let outcome = failure.map_or(DeclarationQueryOutcome::Unavailable, |failure| {
            DeclarationQueryOutcome::Rejected(RejectedDeclarations {
                failure: Arc::new(failure),
            })
        });
        Ok(DeclarationQueryProduct {
            outcome,
            fingerprint: current.fingerprint,
        })
    }
}

/// Demands the declaration product for one published semantic scope.
///
/// # Errors
///
/// Returns only computation-kernel failures. Authored declaration rejection is an ordinary
/// [`DeclarationQueryOutcome`].
pub(crate) fn declarations(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<DeclarationQueryProduct>, ComputationError> {
    database.query::<DeclarationQuery>(key)
}

#[must_use]
fn declaration_execution_count(database: &Database) -> u64 {
    database.execution_count::<DeclarationQuery>()
}

#[must_use]
fn declaration_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<DeclarationQuery>()
}

fn encode(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

/// Publishes one exact source unit and demands its sole closed semantic outcome.
///
/// Intermediate semantic stages remain private to this crate; callers can neither reorder them
/// nor manufacture invalidation fingerprints.
///
/// # Errors
///
/// Returns source-surface validation or computation-kernel failures.
pub(super) fn analyze_unit(
    database: &mut Database,
    unit: Arc<DiscoveredUnit>,
    module_surface_fingerprint: Fingerprint,
    bodies: impl IntoIterator<Item = BodySourcePublication>,
) -> Result<Arc<UnitAnalysisProduct>, SemanticAnalysisError> {
    let (scope, publication) = ScopeInputPublication::for_unit(unit, module_surface_fingerprint)?;
    let mut revision = database.advance_revision()?;
    publication.publish(&mut revision, &scope);
    for body in bodies {
        body.publish(&mut revision);
    }
    let _ = revision.commit();
    unit_analysis::analyzed_unit(database, scope).map_err(SemanticAnalysisError::from)
}

#[derive(Debug)]
pub(super) enum SemanticAnalysisError {
    Computation(ComputationError),
    Input(SemanticInputError),
}

impl std::fmt::Display for SemanticAnalysisError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Computation(error) => error.fmt(formatter),
            Self::Input(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticAnalysisError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Computation(error) => Some(error),
            Self::Input(error) => Some(error),
        }
    }
}

impl From<ComputationError> for SemanticAnalysisError {
    fn from(error: ComputationError) -> Self {
        Self::Computation(error)
    }
}

impl From<SemanticInputError> for SemanticAnalysisError {
    fn from(error: SemanticInputError) -> Self {
        Self::Input(error)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct SemanticComputationStatistics {
    pub declaration_executions: u64,
    pub declaration_reuses: u64,
    pub preparation_executions: u64,
    pub preparation_reuses: u64,
    pub body_name_executions: u64,
    pub body_name_reuses: u64,
    pub typed_body_executions: u64,
    pub typed_body_reuses: u64,
    pub finalization_executions: u64,
    pub finalization_reuses: u64,
    pub complete_analysis_executions: u64,
    pub complete_analysis_reuses: u64,
    pub incomplete_analysis_executions: u64,
    pub incomplete_analysis_reuses: u64,
    pub unit_analysis_executions: u64,
    pub unit_analysis_reuses: u64,
}

#[must_use]
pub(super) fn statistics(database: &Database) -> SemanticComputationStatistics {
    SemanticComputationStatistics {
        declaration_executions: declaration_execution_count(database),
        declaration_reuses: declaration_reuse_count(database),
        preparation_executions: program_preparation::preparation_execution_count(database),
        preparation_reuses: program_preparation::preparation_reuse_count(database),
        body_name_executions: body_names::body_name_execution_count(database),
        body_name_reuses: body_names::body_name_reuse_count(database),
        typed_body_executions: typed_bodies::typed_body_execution_count(database),
        typed_body_reuses: typed_bodies::typed_body_reuse_count(database),
        finalization_executions: program_finalization::finalization_execution_count(database),
        finalization_reuses: program_finalization::finalization_reuse_count(database),
        complete_analysis_executions: program_analysis::program_analysis_execution_count(database),
        complete_analysis_reuses: program_analysis::program_analysis_reuse_count(database),
        incomplete_analysis_executions: incomplete_analysis::incomplete_analysis_execution_count(
            database,
        ),
        incomplete_analysis_reuses: incomplete_analysis::incomplete_analysis_reuse_count(database),
        unit_analysis_executions: unit_analysis::unit_analysis_execution_count(database),
        unit_analysis_reuses: unit_analysis::unit_analysis_reuse_count(database),
    }
}
