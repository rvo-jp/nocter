use nocter_diagnostics::SourceDiagnostic;

use crate::ResolvedBodyNames;

/// The exact lexical-name result retained for one declared body in an editor analysis report.
#[derive(Debug)]
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
#[derive(Debug)]
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

#[derive(Debug)]
enum NameRejectionEvidence {
    None,
    Partial(ResolvedBodyNames),
}
