use nocter_diagnostics::SourceDiagnostic;
use nocter_model::TypeProjection;

use crate::{CheckedBody, TypedBodyInterruption};

/// The exact typed-body result retained for one declared body in an editor analysis report.
///
/// There is no absent state. An authored body either completed typed construction or owns the
/// rejection that explains why typed facts are unavailable.
#[derive(Clone, Debug)]
pub enum BodyEvidence {
    Typed(CheckedBody),
    Rejected(BodyRejection),
}

impl BodyEvidence {
    #[must_use]
    pub const fn typed(&self) -> Option<&CheckedBody> {
        match self {
            Self::Typed(body) => Some(body),
            Self::Rejected(_) => None,
        }
    }

    #[must_use]
    pub const fn rejection(&self) -> Option<&BodyRejection> {
        match self {
            Self::Typed(_) => None,
            Self::Rejected(rejection) => Some(rejection),
        }
    }
}

/// The source-level reason why one declared body has no typed-body evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BodyRejectionReason {
    Authored(SourceDiagnostic),
    IncompleteSyntax,
}

/// One rejected body and the exact recovery capability fixed before rejection.
#[derive(Clone, Debug)]
pub struct BodyRejection {
    reason: BodyRejectionReason,
    recovery: BodyRejectionRecovery,
}

impl BodyRejection {
    pub(crate) const fn new(reason: BodyRejectionReason, recovery: BodyRejectionRecovery) -> Self {
        Self { reason, recovery }
    }

    #[must_use]
    pub const fn reason(&self) -> &BodyRejectionReason {
        &self.reason
    }

    #[must_use]
    pub const fn diagnostic(&self) -> Option<&SourceDiagnostic> {
        match &self.reason {
            BodyRejectionReason::Authored(diagnostic) => Some(diagnostic),
            BodyRejectionReason::IncompleteSyntax => None,
        }
    }

    #[must_use]
    pub const fn interruption(&self) -> Option<&TypedBodyInterruption> {
        match &self.recovery {
            BodyRejectionRecovery::None => None,
            BodyRejectionRecovery::Typed(snapshot) => Some(&snapshot.interruption),
        }
    }

    pub(crate) const fn snapshot(&self) -> Option<&TypedInterruptionSnapshot> {
        match &self.recovery {
            BodyRejectionRecovery::None => None,
            BodyRejectionRecovery::Typed(snapshot) => Some(snapshot),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum BodyRejectionRecovery {
    None,
    Typed(TypedInterruptionSnapshot),
}

impl BodyRejectionRecovery {
    pub(crate) const fn typed(
        interruption: TypedBodyInterruption,
        evidence: TypedInterruptionEvidence,
    ) -> Self {
        Self::Typed(TypedInterruptionSnapshot {
            interruption,
            evidence,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TypedInterruptionSnapshot {
    pub(crate) interruption: TypedBodyInterruption,
    pub(crate) evidence: TypedInterruptionEvidence,
}

/// Exact semantic capability retained for one typed interruption.
#[derive(Clone, Debug)]
pub(crate) enum TypedInterruptionEvidence {
    None,
    MemberSelection(Box<MemberInterruptionEvidence>),
    Outcome(Box<TypeProjection>),
}

#[derive(Clone, Debug)]
pub(crate) struct MemberInterruptionEvidence {
    pub(crate) semantics: crate::semantic_authority::SemanticAuthority,
}
