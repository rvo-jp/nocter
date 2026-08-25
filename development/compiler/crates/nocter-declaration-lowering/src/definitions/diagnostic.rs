use std::fmt;

use nocter_declarations::{DeclarationValidationReport, DeclarationViolation};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use crate::{DiagnosticNote, SourceDiagnostic};

/// Complete source projection of one rejected declaration-validation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationDiagnostics {
    report: DeclarationValidationReport,
    sources: Box<[SourceDiagnostic]>,
}

impl DeclarationDiagnostics {
    pub(super) fn project(
        report: DeclarationValidationReport,
        sources: &SourceIndex,
    ) -> Result<Self, nocter_model::DeclarationSiteId> {
        let mut projected = Vec::with_capacity(report.len());
        for violation in report.violations() {
            projected.push(project_violation(*violation, sources)?);
        }
        projected.sort_by(|left, right| {
            let left_origin = left.primary();
            let right_origin = right.primary();
            (
                left_origin.source(),
                left_origin.span().range().start(),
                left_origin.span().range().end(),
                left.code(),
            )
                .cmp(&(
                    right_origin.source(),
                    right_origin.span().range().start(),
                    right_origin.span().range().end(),
                    right.code(),
                ))
        });
        Ok(Self {
            report,
            sources: projected.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn report(&self) -> &DeclarationValidationReport {
        &self.report
    }

    #[must_use]
    pub const fn sources(&self) -> &[SourceDiagnostic] {
        &self.sources
    }
}

impl fmt::Display for DeclarationDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.report.fmt(formatter)
    }
}

impl std::error::Error for DeclarationDiagnostics {}

fn project_violation(
    violation: DeclarationViolation,
    sources: &SourceIndex,
) -> Result<SourceDiagnostic, nocter_model::DeclarationSiteId> {
    let primary = declaration_origin(sources, violation.primary()).ok_or(violation.primary())?;
    let related = violation
        .related()
        .map(|site| declaration_origin(sources, site).ok_or(site))
        .transpose()?;
    let notes = related
        .zip(violation.rule().related_message())
        .map(|(origin, message)| DiagnosticNote::new(message, origin))
        .into_iter()
        .collect::<Vec<_>>();
    Ok(SourceDiagnostic::new(
        violation.rule().code(),
        violation.rule().message(),
        primary,
        notes,
        Some(violation.rule().help()),
    ))
}

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
