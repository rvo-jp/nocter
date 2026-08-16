use std::fmt;

use nocter_declarations::{DeclarationRule, DeclarationViolation};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use crate::{DiagnosticNote, SourceDiagnostic};

/// One source-backed declaration-rule diagnostic produced at the freeze boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationDiagnostic {
    violation: DeclarationViolation,
    source: Box<SourceDiagnostic>,
}

impl DeclarationDiagnostic {
    pub(super) fn project(
        violation: DeclarationViolation,
        sources: &SourceIndex,
    ) -> Result<Self, nocter_model::DeclarationSiteId> {
        let primary =
            declaration_origin(sources, violation.primary()).ok_or(violation.primary())?;
        let related = violation
            .related()
            .map(|site| declaration_origin(sources, site).ok_or(site))
            .transpose()?;
        let notes = related
            .zip(violation.rule().related_message())
            .map(|(origin, message)| DiagnosticNote::new(message, origin))
            .into_iter()
            .collect::<Vec<_>>();
        let source = SourceDiagnostic::new(
            violation.rule().code(),
            violation.rule().message(),
            primary,
            notes,
            Some(violation.rule().help()),
        );
        Ok(Self {
            violation,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> DeclarationRule {
        self.violation.rule()
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }

    #[must_use]
    pub const fn code(&self) -> &str {
        self.source.code()
    }

    #[must_use]
    pub const fn message(&self) -> &str {
        self.source.message()
    }

    #[must_use]
    pub fn help(&self) -> Option<&str> {
        self.source.help()
    }

    #[must_use]
    pub const fn primary(&self) -> SourceOrigin {
        self.source.primary()
    }

    #[must_use]
    pub fn related(&self) -> Option<SourceOrigin> {
        self.source.notes().first().map(DiagnosticNote::origin)
    }

    #[must_use]
    pub fn related_message(&self) -> Option<&str> {
        self.source.notes().first().map(DiagnosticNote::message)
    }
}

impl fmt::Display for DeclarationDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for DeclarationDiagnostic {}

fn declaration_origin(
    sources: &SourceIndex,
    site: nocter_model::DeclarationSiteId,
) -> Option<SourceOrigin> {
    sources
        .bindings_for(SemanticEntity::DeclarationSite(site))
        .iter()
        .find(|binding| binding.role() == SourceRole::Declaration)
        .map(|binding| binding.origin())
}
