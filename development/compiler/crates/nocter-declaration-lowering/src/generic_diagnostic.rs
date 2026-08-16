use std::fmt;

use crate::{
    CompileUnitInput, GenericRule, GenericViolation, SourceDiagnostic,
    diagnostic::project_syntax_diagnostic,
};

/// One generic-binder rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericDiagnostic {
    rule: GenericRule,
    source: Box<SourceDiagnostic>,
}

impl GenericDiagnostic {
    pub(crate) fn project(
        violation: GenericViolation,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, GenericViolation> {
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
    pub const fn rule(&self) -> GenericRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for GenericDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for GenericDiagnostic {}
