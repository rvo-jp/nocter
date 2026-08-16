use std::fmt;

use nocter_declarations::{DeclarationRule, DeclarationViolation};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

/// One source-backed declaration-rule diagnostic produced at the freeze boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclarationDiagnostic {
    violation: DeclarationViolation,
    primary: SourceOrigin,
    related: Option<SourceOrigin>,
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
        Ok(Self {
            violation,
            primary,
            related,
        })
    }

    #[must_use]
    pub const fn rule(self) -> DeclarationRule {
        self.violation.rule()
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        self.rule().code()
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        self.rule().message()
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        self.rule().help()
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        self.rule().related_message()
    }

    #[must_use]
    pub const fn primary(self) -> SourceOrigin {
        self.primary
    }

    #[must_use]
    pub const fn related(self) -> Option<SourceOrigin> {
        self.related
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
