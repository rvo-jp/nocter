use std::fmt;

use crate::{
    CompileUnitInput, SourceDiagnostic, TypeBindingRule, TypeBindingViolation,
    diagnostic::project_syntax_diagnostic,
};

/// One declaration-header type rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeBindingDiagnostic {
    rule: TypeBindingRule,
    source: Box<SourceDiagnostic>,
}

impl TypeBindingDiagnostic {
    pub(crate) fn project(
        violation: TypeBindingViolation,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, TypeBindingViolation> {
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
    pub const fn rule(&self) -> TypeBindingRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for TypeBindingDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for TypeBindingDiagnostic {}
