use std::fmt;
use std::sync::{Mutex, OnceLock};

use nocter_declarations::DeclarationGraph;
use nocter_model::{
    BodyId, BorrowCapability, CallableId, FieldId, Symbol, TypeId, TypeKind, TypeStore,
};
use nocter_source::SourceId;

use crate::body_check::BodyAssumptionTable;
use crate::field_selection::{FieldSelectionError, select_field};
use crate::instance_operations::{
    InstanceOperationSelector, InstanceSelectionContext, MethodCompletionCandidate,
};
use crate::{
    CheckedProgram, CopyabilityTable, InstanceOperationTable, InterfaceImplementationTable,
    PreparedSemanticProgram,
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
pub struct MemberCompletionContext {
    body: BodyId,
    source: SourceId,
    receiver: TypeId,
    available: BorrowCapability,
    owned: bool,
}

impl MemberCompletionContext {
    #[must_use]
    pub const fn new(
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
            Self::PoisonedQueryState => {
                formatter.write_str("member completion query state is poisoned")
            }
        }
    }
}

impl std::error::Error for MemberCompletionError {}

#[derive(Debug)]
struct MemberCompletionQueryState {
    types: TypeStore,
    copyabilities: CopyabilityTable,
}

/// Lazily forked, query-only semantic state for member completion.
///
/// The canonical program remains immutable. Structural types interned while proving completion
/// candidates and memoized copy conditions are retained across queries in the same generation,
/// avoiding a full program-store clone for every keystroke.
/// Per-generation mutable state used only while answering member-completion queries.
///
/// This session deliberately lives outside [`CheckedProgram`] and [`PreparedSemanticProgram`].
/// Each session belongs to exactly one immutable semantic authority. Callers that retain multiple
/// checked or recovery authorities must retain one session for each authority.
#[derive(Debug, Default)]
pub struct MemberCompletionQuerySession {
    state: OnceLock<Mutex<MemberCompletionQueryState>>,
}

impl MemberCompletionQuerySession {
    fn state<'program>(
        &'program self,
        types: &TypeStore,
        copyabilities: &CopyabilityTable,
    ) -> Result<std::sync::MutexGuard<'program, MemberCompletionQueryState>, MemberCompletionError>
    {
        self.state
            .get_or_init(|| {
                Mutex::new(MemberCompletionQueryState {
                    types: types.clone(),
                    copyabilities: copyabilities.clone(),
                })
            })
            .lock()
            .map_err(|_| MemberCompletionError::PoisonedQueryState)
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
        context: MemberCompletionContext,
    ) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
        select_member_completions(
            MemberCompletionAuthorities {
                graph: self.graph(),
                types: self.types(),
                interface_implementations: self.interface_implementations(),
                instance_operations: self.instance_operations(),
                body_assumptions: self.body_assumptions(),
                copyabilities: self.copyabilities(),
                source_access: self.source_access(),
                session,
            },
            context,
        )
    }
}

impl PreparedSemanticProgram {
    /// Enumerates members from the completed pre-body semantic authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable preparation authorities are inconsistent or
    /// requirement proof cannot be completed for the supplied receiver context.
    pub fn member_completions(
        &self,
        session: &MemberCompletionQuerySession,
        context: MemberCompletionContext,
    ) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
        select_member_completions(
            MemberCompletionAuthorities {
                graph: self.graph(),
                types: self.types(),
                interface_implementations: self.interface_implementations(),
                instance_operations: self.instance_operations(),
                body_assumptions: self.body_assumptions(),
                copyabilities: self.copyabilities(),
                source_access: self.source_access(),
                session,
            },
            context,
        )
    }
}

#[derive(Clone, Copy)]
pub(crate) struct MemberCompletionAuthorities<'program> {
    pub(crate) graph: &'program DeclarationGraph,
    pub(crate) types: &'program TypeStore,
    pub(crate) interface_implementations: &'program InterfaceImplementationTable,
    pub(crate) instance_operations: &'program InstanceOperationTable,
    pub(crate) body_assumptions: &'program BodyAssumptionTable,
    pub(crate) copyabilities: &'program CopyabilityTable,
    pub(crate) source_access: &'program nocter_frontend_bindings::SourceAccessTable,
    pub(crate) session: &'program MemberCompletionQuerySession,
}

pub(crate) fn select_member_completions(
    authorities: MemberCompletionAuthorities<'_>,
    context: MemberCompletionContext,
) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
    let MemberCompletionAuthorities {
        graph,
        types,
        interface_implementations,
        instance_operations,
        body_assumptions,
        copyabilities,
        source_access,
        session,
    } = authorities;
    let mut state = session.state(types, copyabilities)?;
    let MemberCompletionQueryState {
        types,
        copyabilities,
    } = &mut *state;
    let receiver = match types.get(context.receiver) {
        Some(TypeKind::Borrow { referent, .. }) => *referent,
        Some(_) => context.receiver,
        None => return Err(MemberCompletionError::UnknownReceiver(context.receiver)),
    };
    let access = crate::SourceAccessContext::for_source(source_access, context.source)
        .map_err(MemberCompletionError::SourceAccess)?;
    let mut completions = field_completions(graph, types, access, receiver)?;
    let assumptions = body_assumptions
        .get(context.body)
        .ok_or(MemberCompletionError::MissingBody(context.body))?;
    let selection = InstanceSelectionContext::new(
        graph,
        interface_implementations,
        instance_operations,
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
    types: &mut TypeStore,
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
