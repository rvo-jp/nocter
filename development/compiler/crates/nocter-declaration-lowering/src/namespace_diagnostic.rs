use std::fmt;

use crate::{CompileUnitInput, NamespaceRule, NamespaceViolation, SourceDiagnostic};

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
        let trees = crate::diagnostic::input_trees(input).collect::<Vec<_>>();
        Self::project_trees(violation, &trees)
    }

    pub(crate) fn project_surface(
        violation: NamespaceViolation,
        surface: &crate::DeclarationSurface<'_>,
    ) -> Result<Self, NamespaceViolation> {
        let trees = surface
            .sources()
            .iter()
            .map(crate::SurfaceSource::syntax)
            .collect::<Vec<_>>();
        Self::project_trees(violation, &trees)
    }

    fn project_trees(
        violation: NamespaceViolation,
        trees: &[&nocter_syntax::SyntaxTree],
    ) -> Result<Self, NamespaceViolation> {
        let rule = violation.rule();
        let source = crate::diagnostic::project_syntax_diagnostic_from_trees(
            trees,
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
