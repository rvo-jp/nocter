use nocter_diagnostics::SourceDiagnostic;
use nocter_model::BodyId;

use crate::{ResolvedBodyNames, ReusableBodyNames};

/// One independently queried lexical result.
///
/// Accepted names are source-neutral and may be reused across current source generations. A
/// rejection retains its exact current diagnostic while keeping any lexical prefix as a reusable
/// recipe for current-generation recovery materialization.
#[derive(Debug)]
pub enum ReusableBodyNameQueryOutcome {
    Resolved(ReusableBodyNames),
    Rejected(QueriedBodyNameRejection),
}

/// Exact-current authored name rejection plus an optional source-neutral lexical prefix.
#[derive(Debug)]
pub struct QueriedBodyNameRejection {
    body: BodyId,
    diagnostic: SourceDiagnostic,
    partial: Option<ReusableBodyNames>,
}

impl QueriedBodyNameRejection {
    pub(crate) const fn new(
        body: BodyId,
        diagnostic: SourceDiagnostic,
        partial: Option<ReusableBodyNames>,
    ) -> Self {
        Self {
            body,
            diagnostic,
            partial,
        }
    }

    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn diagnostic(&self) -> &SourceDiagnostic {
        &self.diagnostic
    }

    #[must_use]
    pub const fn partial_names(&self) -> Option<&ReusableBodyNames> {
        self.partial.as_ref()
    }
}

/// The exact lexical-name result retained for one declared body in an editor analysis report.
#[derive(Clone, Debug)]
pub enum BodyNameEvidence {
    Resolved(ResolvedBodyNames),
    Rejected(NameRejection),
}

impl BodyNameEvidence {
    /// Returns the complete or partial lexical facts that are safe for local editor queries.
    #[must_use]
    pub const fn usable_names(&self) -> Option<&ResolvedBodyNames> {
        match self {
            Self::Resolved(names) => Some(names),
            Self::Rejected(rejection) => rejection.partial_names(),
        }
    }

    #[must_use]
    pub const fn rejection(&self) -> Option<&NameRejection> {
        match self {
            Self::Resolved(_) => None,
            Self::Rejected(rejection) => Some(rejection),
        }
    }
}

/// One authored name-resolution rejection and the lexical prefix fixed before it.
#[derive(Clone, Debug)]
pub struct NameRejection {
    diagnostic: SourceDiagnostic,
    evidence: NameRejectionEvidence,
}

impl NameRejection {
    pub(crate) fn new(diagnostic: SourceDiagnostic, partial: Option<ResolvedBodyNames>) -> Self {
        Self {
            diagnostic,
            evidence: partial.map_or(NameRejectionEvidence::None, NameRejectionEvidence::Partial),
        }
    }

    #[must_use]
    pub const fn diagnostic(&self) -> &SourceDiagnostic {
        &self.diagnostic
    }

    #[must_use]
    pub const fn partial_names(&self) -> Option<&ResolvedBodyNames> {
        match &self.evidence {
            NameRejectionEvidence::None => None,
            NameRejectionEvidence::Partial(names) => Some(names),
        }
    }
}

#[derive(Clone, Debug)]
enum NameRejectionEvidence {
    None,
    Partial(ResolvedBodyNames),
}
