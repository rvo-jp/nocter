use std::fmt;

use crate::{
    CompileUnitInput, DiagnosticNote, NamespaceRule, NamespaceViolation, SourceDiagnostic,
    diagnostic::{input_trees, origin_from_syntax},
};

/// One namespace rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamespaceDiagnostic {
    rule: NamespaceRule,
    source: Box<SourceDiagnostic>,
}

impl NamespaceDiagnostic {
    pub(crate) fn project(
        violation: NamespaceViolation,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, NamespaceViolation> {
        let primary =
            origin_from_syntax(input_trees(input), violation.primary()).ok_or(violation)?;
        let related = violation
            .related()
            .map(|syntax| origin_from_syntax(input_trees(input), syntax).ok_or(violation))
            .transpose()?;
        let notes = related
            .zip(violation.rule().related_message())
            .map(|(origin, message)| DiagnosticNote::new(message, origin))
            .into_iter()
            .collect::<Vec<_>>();
        let rule = violation.rule();
        let source = SourceDiagnostic::new(
            rule.code(),
            rule.message(),
            primary,
            notes,
            Some(rule.help()),
        );
        Ok(Self {
            rule,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> NamespaceRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for NamespaceDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for NamespaceDiagnostic {}
