//! Demand-owned semantic queries above physical source discovery.

mod body_context;
mod body_names;
mod incomplete_analysis;
mod program_finalization;
mod program_preparation;
mod typed_bodies;

pub use body_names::{
    BodyNameQueryOutcome, BodyNameQueryProduct, BodyNameSet, SemanticBodyKey,
    body_name_execution_count, body_name_reuse_count, resolve_body_name, resolved_body_names,
};
pub use incomplete_analysis::{
    IncompleteAnalysisProduct, IncompleteSemanticAnalysis, IncompleteSemanticError,
    IncompleteSemanticEvidence, IncompleteSemanticFailure, analyze_incomplete_semantics,
    continue_declaration_recovery, incomplete_analysis, incomplete_analysis_execution_count,
    incomplete_analysis_reuse_count,
};
pub use program_finalization::{
    FailedProgramFinalization, FailedProgramNameResolution, FinalizedProgram,
    ProgramFinalizationOutcome, ProgramFinalizationProduct, finalization_execution_count,
    finalization_reuse_count, finalized_program,
};
pub use program_preparation::{
    ProgramPreparationOutcome, ProgramPreparationProduct, RejectedProgramPreparation,
    preparation_execution_count, preparation_reuse_count, prepared_program,
};
pub use typed_bodies::{
    TypedBodyQueryOutcome, TypedBodyQueryProduct, TypedBodySet, typed_bodies, typed_body,
    typed_body_execution_count, typed_body_reuse_count,
};

use std::sync::Arc;

use nocter_computation::{
    ComputationError, ComputationKey, Database, Fingerprint, Input, InputRevision, Query,
    QueryValue,
};
use nocter_declaration_lowering::{
    DeclarationLoweringFailure, ReusableDeclarations, lower_reusable_declarations,
};
use nocter_discovery::DiscoveredUnit;

/// Stable identity of one selected semantic compile scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticScopeKey {
    stable: Box<[u8]>,
}

impl SemanticScopeKey {
    #[must_use]
    pub fn for_unit(unit: &DiscoveredUnit) -> Self {
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
pub struct ScopeInputPublication {
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
    pub fn for_unit(
        unit: Arc<DiscoveredUnit>,
        module_surface_fingerprint: Fingerprint,
    ) -> Result<(SemanticScopeKey, Self), ScopeInputError> {
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
    pub const fn new(
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

    pub fn publish(self, revision: &mut InputRevision<'_>, key: &SemanticScopeKey) {
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
pub enum ScopeInputError {
    SemanticTopology(nocter_discovery::SemanticTopologyError),
    CurrentSource(nocter_discovery::CurrentSourceSurfaceError),
}

impl std::fmt::Display for ScopeInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SemanticTopology(error) => error.fmt(formatter),
            Self::CurrentSource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ScopeInputError {}

impl From<nocter_discovery::SemanticTopologyError> for ScopeInputError {
    fn from(error: nocter_discovery::SemanticTopologyError) -> Self {
        Self::SemanticTopology(error)
    }
}

impl From<nocter_discovery::CurrentSourceSurfaceError> for ScopeInputError {
    fn from(error: nocter_discovery::CurrentSourceSurfaceError) -> Self {
        Self::CurrentSource(error)
    }
}

struct DeclarationScopeInput;
struct CurrentSourceScopeInput;
struct BodySourceInput;

impl Input for DeclarationScopeInput {
    type Key = SemanticScopeKey;
    type Value = ScopeInputValue;
}

impl Input for CurrentSourceScopeInput {
    type Key = SemanticScopeKey;
    type Value = ScopeInputValue;
}

impl Input for BodySourceInput {
    type Key = BodySourceKey;
    type Value = BodySourceValue;
}

/// Stable physical identity of one executable body beneath a declaration surface.
#[derive(Clone, Debug, Eq, PartialEq)]
struct BodySourceKey {
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

struct BodySourceValue {
    fingerprint: Fingerprint,
}

impl QueryValue for BodySourceValue {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

/// One exact per-body input staged with its containing semantic scope revision.
pub struct BodySourcePublication {
    key: BodySourceKey,
    value: BodySourceValue,
}

impl BodySourcePublication {
    #[must_use]
    pub fn new(path: &str, body: &nocter_syntax::BodySyntaxSurface) -> Self {
        Self {
            key: BodySourceKey::new(path, body.locator()),
            value: BodySourceValue {
                fingerprint: Fingerprint::from_bytes(body.canonical_bytes()),
            },
        }
    }

    pub fn publish(self, revision: &mut InputRevision<'_>) {
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

/// One declaration rejection inseparably paired with the exact source domain that produced it.
#[derive(Debug)]
pub struct RejectedDeclarations {
    unit: Arc<DiscoveredUnit>,
    failure: Arc<DeclarationLoweringFailure>,
}

impl RejectedDeclarations {
    #[must_use]
    pub fn unit(&self) -> &Arc<DiscoveredUnit> {
        &self.unit
    }

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
                unit: Arc::clone(&current.unit),
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
pub fn declarations(
    database: &Database,
    key: SemanticScopeKey,
) -> Result<Arc<DeclarationQueryProduct>, ComputationError> {
    database.query::<DeclarationQuery>(key)
}

#[must_use]
pub fn declaration_execution_count(database: &Database) -> u64 {
    database.execution_count::<DeclarationQuery>()
}

#[must_use]
pub fn declaration_reuse_count(database: &Database) -> u64 {
    database.reuse_count::<DeclarationQuery>()
}

fn encode(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}
