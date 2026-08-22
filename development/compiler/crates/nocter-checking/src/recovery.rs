use nocter_model::{ModuleId, TypeStore};

use crate::member_completion::select_member_completions;
use crate::{
    ConstructionCompletionCandidate, ConstructionCompletionError, CopyabilityTable,
    MemberCompletionCandidate, MemberCompletionContext, MemberCompletionError,
    PreparedSemanticProgram, TypedBodyInterruption, TypedBodyInterruptionKind,
};

/// The exact semantic stage retained by one failed editor analysis generation.
#[derive(Debug)]
pub enum SemanticAnalysisRecovery {
    Names(Box<crate::NameAnalysisRecovery>),
    Bodies(Box<BodyAnalysisRecovery>),
}

impl SemanticAnalysisRecovery {
    #[must_use]
    pub fn names(&self) -> Option<&crate::NameAnalysisRecovery> {
        match self {
            Self::Names(recovery) => Some(recovery.as_ref()),
            Self::Bodies(_) => None,
        }
    }

    #[must_use]
    pub fn bodies(&self) -> Option<&BodyAnalysisRecovery> {
        match self {
            Self::Bodies(recovery) => Some(recovery.as_ref()),
            Self::Names(_) => None,
        }
    }
}

#[derive(Debug)]
struct TypedInterruptionSnapshot {
    interruption: TypedBodyInterruption,
    types: TypeStore,
    copyabilities: CopyabilityTable,
}

/// The deepest immutable current-generation semantic state retained after typed-body failure.
///
/// The prepared program remains the authority for declarations, names, and scopes. A typed
/// interruption additionally owns the monotonic type/copyability stores used at the exact failed
/// operation; it never masquerades as a checked body or supplies dispatch for invalid source.
#[derive(Debug)]
pub struct BodyAnalysisRecovery {
    prepared: PreparedSemanticProgram,
    typed: Option<TypedInterruptionSnapshot>,
}

impl BodyAnalysisRecovery {
    pub(crate) fn new(
        prepared: PreparedSemanticProgram,
        typed: Option<(TypedBodyInterruption, TypeStore, CopyabilityTable)>,
    ) -> Self {
        Self {
            prepared,
            typed: typed.map(
                |(interruption, types, copyabilities)| TypedInterruptionSnapshot {
                    interruption,
                    types,
                    copyabilities,
                },
            ),
        }
    }

    #[must_use]
    pub const fn prepared(&self) -> &PreparedSemanticProgram {
        &self.prepared
    }

    #[must_use]
    pub fn interruption(&self) -> Option<TypedBodyInterruption> {
        self.typed.as_ref().map(|typed| typed.interruption)
    }

    /// Applies the normal member selector to an exact failed member-selection context.
    #[must_use]
    pub fn interrupted_member_completions(
        &self,
        module: ModuleId,
    ) -> Option<Result<Box<[MemberCompletionCandidate]>, MemberCompletionError>> {
        let typed = self.typed.as_ref()?;
        let TypedBodyInterruptionKind::MemberSelection {
            receiver,
            available,
            owned,
        } = typed.interruption.kind()
        else {
            return None;
        };
        let body = typed.interruption.body();
        let owner = match self.prepared.graph().declarations().bodies().get(body) {
            Some(body) => body.owner(),
            None => return Some(Err(MemberCompletionError::MissingBody(body))),
        };
        Some(select_member_completions(
            self.prepared.graph(),
            &typed.types,
            self.prepared.conformances(),
            self.prepared.instance_operations(),
            &typed.copyabilities,
            MemberCompletionContext::new(owner, module, receiver, available, owned),
        ))
    }

    /// Applies the use-site construction selector to an exact failed construction selection.
    #[must_use]
    pub fn interrupted_construction_completions(
        &self,
        module: ModuleId,
    ) -> Option<Result<Box<[ConstructionCompletionCandidate]>, ConstructionCompletionError>> {
        let typed = self.typed.as_ref()?;
        let TypedBodyInterruptionKind::ConstructionSelection { owner } = typed.interruption.kind()
        else {
            return None;
        };
        Some(self.prepared.construction_completions(owner, module))
    }
}
