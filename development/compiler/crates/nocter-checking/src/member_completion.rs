use std::fmt;
use std::sync::{Mutex, OnceLock};

use nocter_declarations::DeclarationGraph;
use nocter_model::{
    BodyId, BodyNodeId, BorrowCapability, CallableId, FieldId, Symbol, TypeId, TypeKind,
};
use nocter_source::SourceId;

use crate::CheckedProgram;
use crate::field_selection::{FieldSelectionError, select_field};
use crate::instance_operations::{
    InstanceOperationSelector, InstanceSelectionContext, MethodCompletionCandidate,
};

/// One compiler-selected field or method visible on a receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberCompletionCandidate {
    name: Symbol,
    target: MemberCompletionTarget,
}

impl MemberCompletionCandidate {
    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn target(self) -> MemberCompletionTarget {
        self.target
    }
}

/// The canonical semantic identity represented by a member completion item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberCompletionTarget {
    Field(FieldId),
    Method { surface: Option<CallableId> },
}

/// Exact checked receiver facts required to enumerate callable members.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MemberCompletionContext {
    body: BodyId,
    source: SourceId,
    receiver: TypeId,
    available: BorrowCapability,
    owned: bool,
}

impl MemberCompletionContext {
    #[must_use]
    pub(crate) const fn new(
        body: BodyId,
        source: SourceId,
        receiver: TypeId,
        available: BorrowCapability,
        can_consume: bool,
    ) -> Self {
        Self {
            body,
            source,
            receiver,
            available,
            owned: can_consume,
        }
    }
}

/// Failure to apply the ordinary compiler member-selection authority as a tooling query.
#[derive(Debug)]
pub enum MemberCompletionError {
    SourceAccess(nocter_frontend_bindings::SourceAccessError),
    FieldSelection,
    Selection(crate::InstanceSelectionError),
    MissingBody(BodyId),
    MissingReceiver(nocter_model::BodyNodeId),
    UnknownReceiver(TypeId),
    InvalidRecoveryEvidence,
    AuthorityMismatch,
    PoisonedQueryState,
}

impl fmt::Display for MemberCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceAccess(error) => error.fmt(formatter),
            Self::FieldSelection => formatter.write_str("member field selection is inconsistent"),
            Self::Selection(error) => error.fmt(formatter),
            Self::MissingBody(body) => {
                write!(formatter, "member completion body {body:?} is absent")
            }
            Self::MissingReceiver(receiver) => {
                write!(
                    formatter,
                    "member completion receiver node {receiver:?} is absent"
                )
            }
            Self::UnknownReceiver(receiver) => {
                write!(
                    formatter,
                    "member completion receiver {receiver:?} is absent"
                )
            }
            Self::InvalidRecoveryEvidence => {
                formatter.write_str("member completion recovery evidence is inconsistent")
            }
            Self::AuthorityMismatch => formatter
                .write_str("member completion query session belongs to another semantic authority"),
            Self::PoisonedQueryState => {
                formatter.write_str("member completion query state is poisoned")
            }
        }
    }
}

impl std::error::Error for MemberCompletionError {}

#[derive(Debug)]
struct MemberCompletionQueryState {
    semantics: crate::semantic_authority::SemanticTransaction,
}

/// Lazily forked, query-only semantic state for member completion.
///
/// The canonical program remains immutable. Structural types interned while proving completion
/// candidates and memoized copy conditions are retained across queries in the same generation,
/// avoiding a full program-store clone for every keystroke.
/// Per-generation mutable state used only while answering member-completion queries.
///
/// This session deliberately lives outside [`CheckedProgram`] and
/// [`crate::PreparedSemanticProgram`].
/// Each session belongs to exactly one immutable semantic authority. Callers that retain multiple
/// checked or recovery authorities must retain one session for each authority.
#[derive(Debug, Default)]
pub struct MemberCompletionQuerySession {
    state: OnceLock<Mutex<MemberCompletionQueryState>>,
}

impl MemberCompletionQuerySession {
    fn state<'program>(
        &'program self,
        semantics: &crate::semantic_authority::SemanticAuthority,
    ) -> Result<std::sync::MutexGuard<'program, MemberCompletionQueryState>, MemberCompletionError>
    {
        let state = self
            .state
            .get_or_init(|| {
                Mutex::new(MemberCompletionQueryState {
                    semantics: semantics.transaction(),
                })
            })
            .lock()
            .map_err(|_| MemberCompletionError::PoisonedQueryState)?;
        if !state.semantics.is_based_on(semantics) {
            return Err(MemberCompletionError::AuthorityMismatch);
        }
        Ok(state)
    }
}

impl CheckedProgram {
    /// Enumerates members using the same normalized authorities as ordinary call checking.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable program authorities are inconsistent or requirement
    /// proof cannot be completed for the supplied checked receiver context.
    pub fn member_completions(
        &self,
        session: &MemberCompletionQuerySession,
        body: BodyId,
        receiver: BodyNodeId,
        available: BorrowCapability,
        can_consume: bool,
    ) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
        let checked_body = self
            .bodies()
            .get(body)
            .ok_or(MemberCompletionError::MissingBody(body))?;
        let source = checked_body.source();
        let receiver = checked_body
            .nodes()
            .get(receiver)
            .ok_or(MemberCompletionError::MissingReceiver(receiver))?
            .ty();
        select_member_completions(
            MemberCompletionAuthorities {
                environment: self.environment(),
                semantics: self.semantic_authority(),
                session,
            },
            MemberCompletionContext::new(body, source, receiver, available, can_consume),
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MemberCompletionAuthorities<'program> {
    pub(crate) environment: &'program crate::program_environment::ProgramEnvironment,
    pub(crate) semantics: &'program crate::semantic_authority::SemanticAuthority,
    pub(crate) session: &'program MemberCompletionQuerySession,
}

pub(crate) fn select_member_completions(
    authorities: MemberCompletionAuthorities<'_>,
    context: MemberCompletionContext,
) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
    let MemberCompletionAuthorities {
        environment,
        semantics,
        session,
    } = authorities;
    let graph = environment.graph();
    let mut state = session.state(semantics)?;
    let (types, copyabilities) = state.semantics.access().into_reasoning_parts();
    let receiver = match types.get(context.receiver) {
        Some(TypeKind::Borrow { referent, .. }) => *referent,
        Some(_) => context.receiver,
        None => return Err(MemberCompletionError::UnknownReceiver(context.receiver)),
    };
    let access =
        crate::SourceAccessContext::for_source(environment.source_access(), context.source)
            .map_err(MemberCompletionError::SourceAccess)?;
    let mut completions = field_completions(graph, types, access, receiver)?;
    let assumptions = environment
        .body_assumptions()
        .get(context.body)
        .ok_or(MemberCompletionError::MissingBody(context.body))?;
    let selection = InstanceSelectionContext::new(
        graph,
        environment.interface_implementations(),
        environment.instance_operations(),
        assumptions.declared(),
        assumptions.intrinsic(),
        access,
    );
    let methods = InstanceOperationSelector::new(selection, types, copyabilities)
        .select_member_completions(receiver, context.available, context.owned)
        .map_err(MemberCompletionError::Selection)?;
    completions.extend(
        methods
            .into_iter()
            .map(
                |MethodCompletionCandidate { name, surface }| MemberCompletionCandidate {
                    name,
                    target: MemberCompletionTarget::Method { surface },
                },
            ),
    );
    completions.sort_unstable_by_key(|candidate| {
        (
            candidate.name,
            match candidate.target {
                MemberCompletionTarget::Field(_) => 0_u8,
                MemberCompletionTarget::Method { .. } => 1,
            },
        )
    });
    Ok(completions.into_boxed_slice())
}

fn field_completions(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    access: crate::SourceAccessContext<'_>,
    receiver: TypeId,
) -> Result<Vec<MemberCompletionCandidate>, MemberCompletionError> {
    let names = match types.get(receiver) {
        Some(TypeKind::Nominal { definition, .. }) => {
            let Some(nominal) = graph.declarations().nominal_types().get(*definition) else {
                return Err(MemberCompletionError::FieldSelection);
            };
            let nocter_declarations::NominalShape::Struct { fields, .. } = nominal.shape() else {
                return Ok(Vec::new());
            };
            fields
                .iter()
                .filter_map(|field| {
                    graph
                        .declarations()
                        .fields()
                        .get(*field)
                        .map(|field| field.name())
                })
                .collect::<Vec<_>>()
        }
        Some(_) => return Ok(Vec::new()),
        None => return Err(MemberCompletionError::UnknownReceiver(receiver)),
    };
    let mut completions = Vec::new();
    for name in names {
        let Some(spelling) = graph.symbols().spelling(name) else {
            return Err(MemberCompletionError::FieldSelection);
        };
        match select_field(graph, types, access, receiver, spelling) {
            Ok(field) => completions.push(MemberCompletionCandidate {
                name,
                target: MemberCompletionTarget::Field(field.field()),
            }),
            Err(
                FieldSelectionError::NoFields(_)
                | FieldSelectionError::MissingField(_)
                | FieldSelectionError::InaccessibleField(_),
            ) => {}
            Err(
                FieldSelectionError::UnknownType(_)
                | FieldSelectionError::UnknownNominal(_)
                | FieldSelectionError::UnknownField(_)
                | FieldSelectionError::UnknownFieldSite(_)
                | FieldSelectionError::AmbiguousField(_)
                | FieldSelectionError::GenericArity(_)
                | FieldSelectionError::Substitution(_)
                | FieldSelectionError::SourceAccess(_),
            ) => return Err(MemberCompletionError::FieldSelection),
        }
    }
    Ok(completions)
}

#[cfg(test)]
mod tests {
    use crate::semantic_authority::SemanticAuthority;

    use super::{MemberCompletionError, MemberCompletionQuerySession};

    #[test]
    fn query_session_rejects_reuse_with_another_semantic_authority() {
        let semantics = SemanticAuthority::default();
        let session = MemberCompletionQuerySession::default();
        drop(session.state(&semantics).unwrap());

        let foreign = SemanticAuthority::default();
        assert!(matches!(
            session.state(&foreign),
            Err(MemberCompletionError::AuthorityMismatch)
        ));
    }

    #[test]
    fn query_session_accepts_immutable_clones_of_the_same_authority() {
        let semantics = SemanticAuthority::default();
        let session = MemberCompletionQuerySession::default();
        drop(session.state(&semantics).unwrap());

        drop(session.state(&semantics.clone()).unwrap());
    }
}
