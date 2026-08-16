use std::fmt;

use crate::{
    CompileUnitInput, ImportRule, ImportViolation, SourceDiagnostic,
    diagnostic::project_syntax_diagnostic,
};

/// One import rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDiagnostic {
    rule: ImportRule,
    source: Box<SourceDiagnostic>,
}

impl ImportDiagnostic {
    pub(crate) fn project(
        violation: ImportViolation,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, ImportViolation> {
        let rule = violation.rule();
        let source = project_syntax_diagnostic(
            input,
            violation.primary(),
            violation.related(),
            rule.code(),
            rule.message(),
            rule.related_message(),
            rule.help(),
        )
        .ok_or(violation)?;
        Ok(Self {
            rule,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> ImportRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for ImportDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for ImportDiagnostic {}
