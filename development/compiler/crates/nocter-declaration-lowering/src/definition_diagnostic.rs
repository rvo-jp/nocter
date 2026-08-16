use std::fmt;

use crate::{
    CompileUnitInput, DefinitionRule, DefinitionViolation, SourceDiagnostic,
    diagnostic::project_syntax_diagnostic,
};

/// One declaration-definition rule projected to its exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionDiagnostic {
    rule: DefinitionRule,
    source: Box<SourceDiagnostic>,
}

impl DefinitionDiagnostic {
    pub(crate) fn project(
        violation: DefinitionViolation,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, DefinitionViolation> {
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
    pub const fn rule(&self) -> DefinitionRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for DefinitionDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for DefinitionDiagnostic {}
