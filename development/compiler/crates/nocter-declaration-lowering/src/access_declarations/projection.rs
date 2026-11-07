use std::collections::BTreeMap;

use nocter_declarations::value_access::{DeclarationRule, DeclarationViolation, HeaderPosition};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::AccessCallableId;
use nocter_source_index::SourceOrigin;
use nocter_syntax::{SyntaxToken, SyntaxTree};

use super::{CallableSource, Failure};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeaderDiagnostic {
    violation: DeclarationViolation,
    source: SourceDiagnostic,
}

impl HeaderDiagnostic {
    #[must_use]
    pub const fn violation(&self) -> DeclarationViolation {
        self.violation
    }
    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

pub(super) fn project_violations(
    tree: &SyntaxTree,
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    violations: Vec<DeclarationViolation>,
) -> Failure {
    let projected = violations
        .into_iter()
        .map(|violation| project(tree, sources, violation))
        .collect::<Option<Vec<_>>>();
    let Some(mut diagnostics) = projected else {
        return Failure::InconsistentSyntax;
    };
    diagnostics.sort_by_key(|diagnostic| diagnostic.source.primary().span().range().start());
    Failure::Headers(diagnostics)
}

fn project(
    tree: &SyntaxTree,
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    violation: DeclarationViolation,
) -> Option<HeaderDiagnostic> {
    let source = sources.get(&violation.callable)?;
    let primary = match violation.position {
        HeaderPosition::Name => source.name,
        HeaderPosition::Parameter(index) => *source.parameters.get(index)?,
        HeaderPosition::Result => return None, // No authored valid core syntax can select an unsupported data shape.
    };
    let (code, message, related) = match violation.rule {
        DeclarationRule::DuplicateCallable { first } => (
            DiagnosticCode::E0241,
            "duplicate function name",
            Some(sources.get(&first)?.name),
        ),
        DeclarationRule::DuplicateParameter { first } => (
            DiagnosticCode::E0341,
            "duplicate parameter name",
            Some(*source.parameters.get(first)?),
        ),
        DeclarationRule::ParameterShadowsCallable { declaration } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored function",
            Some(sources.get(&declaration)?.name),
        ),
        DeclarationRule::ReservedBuiltinName => (
            if violation.position == HeaderPosition::Name {
                DiagnosticCode::E0240
            } else {
                DiagnosticCode::E0341
            },
            "name is reserved for a built-in type",
            None,
        ),
        DeclarationRule::MissingDefinition
        | DeclarationRule::UnknownSymbol
        | DeclarationRule::UnsupportedDataType { .. } => return None,
    };
    let primary = origin(tree, primary)?;
    let notes = if let Some(token) = related {
        Some(DiagnosticNote::new(
            if matches!(
                violation.rule,
                DeclarationRule::ParameterShadowsCallable { .. }
            ) {
                "conflicting function is declared here"
            } else {
                "first declaration is here"
            },
            origin(tree, token)?,
        ))
    } else {
        None
    };
    let source = SourceDiagnostic::new(
        code,
        message,
        primary,
        notes.into_iter().collect::<Vec<_>>(),
        None::<&str>,
    );
    Some(HeaderDiagnostic { violation, source })
}

fn origin(tree: &SyntaxTree, token: SyntaxToken) -> Option<SourceOrigin> {
    SourceOrigin::from_token(tree, token).ok()
}
