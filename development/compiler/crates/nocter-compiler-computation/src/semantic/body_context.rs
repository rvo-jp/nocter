//! Current-generation context shared by body queries.

use std::sync::Arc;

use nocter_computation::{ComputationError, Database, Fingerprint, Query, QueryValue};

use super::{CurrentSourceScopeInput, ProgramPreparationOutcome, SemanticScopeKey};

pub(super) struct BodySemanticContextQuery;

struct BodySemanticContext {
    unit: Arc<nocter_discovery::DiscoveredUnit>,
    checking: nocter_checking::ProgramBodyCheckingContext,
}

pub(super) struct BodySemanticContextProduct {
    state: BodySemanticContextState,
    fingerprint: Fingerprint,
}

enum BodySemanticContextState {
    Ready(Box<BodySemanticContext>),
    Failed(Arc<super::SemanticQueryFailure>),
}

impl BodySemanticContextProduct {
    fn queried_name_inputs(
        body_names: &super::BodyNameSet,
    ) -> (
        Vec<&nocter_checking::ReusableBodyNames>,
        Vec<&nocter_checking::QueriedBodyNameRejection>,
    ) {
        (
            body_names.entries().iter().map(AsRef::as_ref).collect(),
            body_names.rejections().iter().map(AsRef::as_ref).collect(),
        )
    }

    /// Resolves only after the exact body input has been demanded by the calling body query.
    pub(super) fn resolve_names(
        &self,
        _exact_body: &super::BodySourceValue,
        identity: &nocter_declaration_lowering::ReusableBodyIdentity,
    ) -> Result<nocter_checking::ReusableBodyNameQueryOutcome, Arc<super::SemanticQueryFailure>>
    {
        let context = match &self.state {
            BodySemanticContextState::Ready(context) => context,
            BodySemanticContextState::Failed(failure) => return Err(Arc::clone(failure)),
        };
        let input = context
            .unit
            .compile_input()
            .map_err(|error| Arc::new(error.into()))?;
        context
            .checking
            .resolve_names(&input, identity.body())
            .map_err(|error| Arc::new(error.into()))
    }

    /// Types only after the exact body and its source-neutral lexical result have been demanded.
    pub(super) fn check_body(
        &self,
        _exact_body: &super::BodySourceValue,
        names: &nocter_checking::ReusableBodyNames,
    ) -> Result<nocter_checking::ReusableBodyQueryOutcome, Arc<super::SemanticQueryFailure>> {
        let context = match &self.state {
            BodySemanticContextState::Ready(context) => context,
            BodySemanticContextState::Failed(failure) => return Err(Arc::clone(failure)),
        };
        let input = context
            .unit
            .compile_input()
            .map_err(|error| Arc::new(error.into()))?;
        context
            .checking
            .check(&input, names)
            .map_err(|error| Arc::new(error.into()))
    }

    pub(super) fn finalize(
        &self,
        body_names: &super::BodyNameSet,
        typed_bodies: &super::TypedBodySet,
    ) -> Result<nocter_checking::QueriedProgramFinalizationOutcome, Arc<super::SemanticQueryFailure>>
    {
        let context = match &self.state {
            BodySemanticContextState::Ready(context) => context,
            BodySemanticContextState::Failed(failure) => return Err(Arc::clone(failure)),
        };
        let input = context
            .unit
            .compile_input()
            .map_err(|error| Arc::new(error.into()))?;
        let (names, name_rejections) = Self::queried_name_inputs(body_names);
        let bodies = typed_bodies
            .entries()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        let body_rejections = typed_bodies
            .rejections()
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>();
        context
            .checking
            .finalize(&input, &names, &name_rejections, &bodies, &body_rejections)
            .map_err(|error| Arc::new(super::SemanticQueryFailure::ProgramFinalization(error)))
    }

    pub(super) fn materialize_name_rejection(
        &self,
        body_names: &super::BodyNameSet,
    ) -> Result<nocter_checking::QueriedNameResolutionFailure, Arc<super::SemanticQueryFailure>>
    {
        let context = match &self.state {
            BodySemanticContextState::Ready(context) => context,
            BodySemanticContextState::Failed(failure) => return Err(Arc::clone(failure)),
        };
        let input = context
            .unit
            .compile_input()
            .map_err(|error| Arc::new(error.into()))?;
        let (names, rejections) = Self::queried_name_inputs(body_names);
        let failure = context
            .checking
            .prepare_names(&input, &names, &rejections)
            .err()
            .ok_or_else(|| Arc::new(super::SemanticQueryFailure::UnexpectedAcceptedNameCatalog))?;
        nocter_checking::QueriedNameResolutionFailure::from_preparation_failure(failure).map_err(
            |failure| {
                Arc::new(super::SemanticQueryFailure::NameRejectionMaterialization(
                    failure,
                ))
            },
        )
    }
}

impl QueryValue for BodySemanticContextProduct {
    fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }
}

impl Query for BodySemanticContextQuery {
    type Key = SemanticScopeKey;
    type Value = BodySemanticContextProduct;

    fn execute(database: &Database, key: &Self::Key) -> Result<Self::Value, ComputationError> {
        let current = database.input::<CurrentSourceScopeInput>(key)?;
        let preparation = super::prepared_program(database, key.clone())?;
        let preparation_fingerprint = preparation.fingerprint();
        let prepared = match preparation.outcome() {
            ProgramPreparationOutcome::Prepared(prepared) => prepared,
            ProgramPreparationOutcome::Failed(failure) => {
                return Ok(BodySemanticContextProduct {
                    state: BodySemanticContextState::Failed(Arc::clone(failure)),
                    fingerprint: current.fingerprint,
                });
            }
            ProgramPreparationOutcome::Rejected(_) => {
                return Ok(BodySemanticContextProduct {
                    state: BodySemanticContextState::Failed(Arc::new(
                        super::SemanticQueryFailure::InvalidStageTransition(
                            "body context demanded after preparation rejection",
                        ),
                    )),
                    fingerprint: current.fingerprint,
                });
            }
        };
        let state = match current.unit.compile_input() {
            Ok(input) => match prepared.open_current(&input) {
                Ok(checking) => BodySemanticContextState::Ready(Box::new(BodySemanticContext {
                    unit: Arc::clone(&current.unit),
                    checking,
                })),
                Err(error) => BodySemanticContextState::Failed(Arc::new(error.into())),
            },
            Err(error) => BodySemanticContextState::Failed(Arc::new(error.into())),
        };
        let fingerprint = if matches!(state, BodySemanticContextState::Ready(_)) {
            preparation_fingerprint
        } else {
            current.fingerprint
        };
        Ok(BodySemanticContextProduct { state, fingerprint })
    }
}
