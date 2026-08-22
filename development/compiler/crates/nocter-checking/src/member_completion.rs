use std::fmt;

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{
    BodyId, BorrowCapability, BuiltinType, CallableId, FieldIdentity, ModuleId, Symbol, TypeId,
    TypeKind, TypeStore,
};

use crate::body_check::body_assumptions;
use crate::field_selection::{FieldSelectionError, select_field};
use crate::instance_operations::{
    InstanceOperationSelector, InstanceSelectionContext, MethodCompletionCandidate,
};
use crate::{
    CheckedProgram, ConformanceTable, CopyabilityTable, InstanceOperationTable,
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
    Field(FieldIdentity),
    Method { surface: Option<CallableId> },
}

/// Exact checked receiver facts required to enumerate callable members.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberCompletionContext {
    owner: BodyOwner,
    module: ModuleId,
    receiver: TypeId,
    available: BorrowCapability,
    owned: bool,
}

impl MemberCompletionContext {
    #[must_use]
    pub const fn new(
        owner: BodyOwner,
        module: ModuleId,
        receiver: TypeId,
        available: BorrowCapability,
        can_consume: bool,
    ) -> Self {
        Self {
            owner,
            module,
            receiver,
            available,
            owned: can_consume,
        }
    }
}

/// Failure to apply the ordinary compiler member-selection authority as a tooling query.
#[derive(Debug)]
pub enum MemberCompletionError {
    Assumptions(crate::SubstitutionError),
    FieldSelection,
    Selection(crate::InstanceSelectionError),
    MissingBody(BodyId),
    MissingReceiver(nocter_model::BodyNodeId),
    UnknownReceiver(TypeId),
}

impl fmt::Display for MemberCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assumptions(error) => error.fmt(formatter),
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
        }
    }
}

impl std::error::Error for MemberCompletionError {}

impl CheckedProgram {
    /// Enumerates members using the same normalized authorities as ordinary call checking.
    ///
    /// # Errors
    ///
    /// Returns an error when the immutable program authorities are inconsistent or requirement
    /// proof cannot be completed for the supplied checked receiver context.
    pub fn member_completions(
        &self,
        context: MemberCompletionContext,
    ) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
        select_member_completions(
            self.graph(),
            self.types(),
            self.conformances(),
            self.instance_operations(),
            self.copyabilities(),
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
        context: MemberCompletionContext,
    ) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
        select_member_completions(
            self.graph(),
            self.types(),
            self.conformances(),
            self.instance_operations(),
            self.copyabilities(),
            context,
        )
    }
}

pub(crate) fn select_member_completions(
    graph: &DeclarationGraph,
    types: &TypeStore,
    conformances: &ConformanceTable,
    instance_operations: &InstanceOperationTable,
    copyabilities: &CopyabilityTable,
    context: MemberCompletionContext,
) -> Result<Box<[MemberCompletionCandidate]>, MemberCompletionError> {
    let mut types = types.clone();
    let mut copyabilities = copyabilities.clone();
    let receiver = match types.get(context.receiver) {
        Some(TypeKind::Borrow { referent, .. }) => *referent,
        Some(_) => context.receiver,
        None => return Err(MemberCompletionError::UnknownReceiver(context.receiver)),
    };
    let mut completions = field_completions(graph, &mut types, context.module, receiver)?;
    let assumptions = body_assumptions(
        graph,
        &mut types,
        conformances,
        instance_operations,
        context.owner,
    )
    .map_err(MemberCompletionError::Assumptions)?;
    let selection = InstanceSelectionContext::new(
        graph,
        conformances,
        instance_operations,
        assumptions.declared(),
        assumptions.intrinsic(),
        context.module,
    );
    let methods = InstanceOperationSelector::new(selection, &mut types, &mut copyabilities)
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
    module: ModuleId,
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
        Some(TypeKind::Builtin(BuiltinType::Error)) => ["code", "message"]
            .into_iter()
            .filter_map(|name| graph.symbols().get(name))
            .collect(),
        Some(_) => return Ok(Vec::new()),
        None => return Err(MemberCompletionError::UnknownReceiver(receiver)),
    };
    let mut completions = Vec::new();
    for name in names {
        let Some(spelling) = graph.symbols().spelling(name) else {
            return Err(MemberCompletionError::FieldSelection);
        };
        match select_field(graph, types, module, receiver, spelling) {
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
                | FieldSelectionError::UnknownBorrowType(_),
            ) => return Err(MemberCompletionError::FieldSelection),
        }
    }
    Ok(completions)
}
