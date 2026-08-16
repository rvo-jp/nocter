use std::fmt;

use crate::{
    CompileUnitInput, SourceDiagnostic, TopologyRule, TopologyViolation,
    diagnostic::project_syntax_diagnostic,
};

/// One authored topology rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyDiagnostic {
    rule: TopologyRule,
    source: Box<SourceDiagnostic>,
}

impl TopologyDiagnostic {
    pub(crate) fn project(
        violation: &TopologyViolation,
        input: &CompileUnitInput<'_>,
    ) -> Option<Self> {
        let rule = violation.rule();
        let source = project_syntax_diagnostic(
            input,
            violation.primary(),
            violation.related().iter().copied(),
            rule.code(),
            rule.message(),
            rule.related_message(),
            rule.help(),
        )?;
        Some(Self {
            rule,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> TopologyRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for TopologyDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for TopologyDiagnostic {}
