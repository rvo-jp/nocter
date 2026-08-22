use std::fmt;

use nocter_declarations::{BodyOwner, DeclarationGraph};
use nocter_model::{BodyId, BorrowCapability, ModuleId, TypeId, TypeKind, TypeStore};

use crate::body_check::body_assumptions;
use crate::instance_operations::{InstanceOperationSelector, InstanceSelectionContext};
use crate::{
    CheckedProgram, ConformanceTable, CopyabilityTable, InstanceOperationTable,
    MemberCompletionCandidate, PreparedSemanticProgram,
};

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
    Selection(crate::InstanceSelectionError),
    MissingBody(BodyId),
    UnknownReceiver(TypeId),
}

impl fmt::Display for MemberCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assumptions(error) => error.fmt(formatter),
            Self::Selection(error) => error.fmt(formatter),
            Self::MissingBody(body) => {
                write!(formatter, "member completion body {body:?} is absent")
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
    InstanceOperationSelector::new(selection, &mut types, &mut copyabilities)
        .select_member_completions(receiver, context.available, context.owned)
        .map(Vec::into_boxed_slice)
        .map_err(MemberCompletionError::Selection)
}
