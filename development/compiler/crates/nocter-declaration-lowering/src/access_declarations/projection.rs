use super::{CallableSource, Failure, RecordHeader};
use nocter_declarations::value_access::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition,
};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessCallableId, NominalTypeId};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{SyntaxOrigin, SyntaxToken, SyntaxTree};
use std::collections::BTreeMap;

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
    records: &BTreeMap<NominalTypeId, RecordHeader>,
    violations: Vec<DeclarationViolation>,
) -> Failure {
    let projected = violations
        .into_iter()
        .map(|v| project(tree, sources, records, v))
        .collect::<Option<Vec<_>>>();
    let Some(mut diagnostics) = projected else {
        return Failure::InconsistentSyntax;
    };
    diagnostics.sort_by_key(|d| d.source.primary().span().range().start());
    Failure::Headers(diagnostics)
}
fn project(
    tree: &SyntaxTree,
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
    violation: DeclarationViolation,
) -> Option<HeaderDiagnostic> {
    use DeclarationRule as R;
    let typed = matches!(
        violation.rule,
        R::UnknownDataName { .. }
            | R::UnsupportedDataType { .. }
            | R::NonCopyField { .. }
            | R::RecursivePayload { .. }
    );
    let primary = primary_source(sources, records, violation, typed)?;
    let (code, message, related) = details(sources, records, violation)?;
    let primary = origin(tree, primary)?;
    let notes = if let Some(token) = related {
        Some(DiagnosticNote::new(
            "related declaration is here",
            origin(tree, SyntaxOrigin::Token(token))?,
        ))
    } else {
        None
    };
    Some(HeaderDiagnostic {
        violation,
        source: SourceDiagnostic::new(
            code,
            message,
            primary,
            notes.into_iter().collect::<Vec<_>>(),
            None::<&str>,
        ),
    })
}
fn primary_source(
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
    violation: DeclarationViolation,
    typed: bool,
) -> Option<SyntaxOrigin> {
    Some(match violation.declaration {
        DeclarationId::Callable(id) => {
            let s = sources.get(&id)?;
            match violation.position {
                HeaderPosition::Name => SyntaxOrigin::Token(s.name),
                HeaderPosition::Parameter(i) if typed => {
                    SyntaxOrigin::Node(*s.parameter_types.get(i)?)
                }
                HeaderPosition::Parameter(i) => SyntaxOrigin::Token(*s.parameters.get(i)?),
                HeaderPosition::Result => SyntaxOrigin::Node(s.result),
                HeaderPosition::Field(_) => return None,
            }
        }
        DeclarationId::Record(id) => {
            let s = records.get(&id)?;
            match violation.position {
                HeaderPosition::Name => SyntaxOrigin::Token(s.name),
                HeaderPosition::Field(i) if typed => SyntaxOrigin::Node(s.fields.get(i)?.2),
                HeaderPosition::Field(i) => SyntaxOrigin::Token(s.fields.get(i)?.0),
                _ => return None,
            }
        }
    })
}
fn details(
    sources: &BTreeMap<AccessCallableId, CallableSource>,
    records: &BTreeMap<NominalTypeId, RecordHeader>,
    violation: DeclarationViolation,
) -> Option<(DiagnosticCode, &'static str, Option<SyntaxToken>)> {
    use DeclarationRule as R;
    Some(match violation.rule {
        R::InvalidConstantHeader => (
            DiagnosticCode::E0323,
            "constant headers require a data value and no inputs",
            None,
        ),
        R::DuplicateCallable { first } => (
            DiagnosticCode::E0241,
            "duplicate value declaration name",
            Some(sources.get(&first)?.name),
        ),
        R::DuplicateRecord { first } => (
            DiagnosticCode::E0241,
            "duplicate record name",
            Some(records.get(&first)?.name),
        ),
        R::RecordShadowsCallable { declaration } => (
            DiagnosticCode::E0241,
            "record name conflicts with a value declaration",
            Some(sources.get(&declaration)?.name),
        ),
        R::DuplicateParameter { first } => {
            let DeclarationId::Callable(id) = violation.declaration else {
                return None;
            };
            (
                DiagnosticCode::E0341,
                "duplicate parameter name",
                Some(*sources.get(&id)?.parameters.get(first)?),
            )
        }
        R::DuplicateField { first } => {
            let DeclarationId::Record(id) = violation.declaration else {
                return None;
            };
            (
                DiagnosticCode::E0241,
                "duplicate field name",
                Some(records.get(&id)?.fields.get(first)?.0),
            )
        }
        R::ParameterShadowsCallable { declaration } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored value declaration",
            Some(sources.get(&declaration)?.name),
        ),
        R::ParameterShadowsRecord { declaration } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored type",
            Some(records.get(&declaration)?.name),
        ),
        R::ReservedBuiltinName => (
            if violation.position == HeaderPosition::Name {
                DiagnosticCode::E0240
            } else {
                DiagnosticCode::E0341
            },
            "name is reserved for a built-in type",
            None,
        ),
        R::UnknownDataName { .. } => (DiagnosticCode::E0290, "unknown data type", None),
        R::UnsupportedDataType { .. } => (
            DiagnosticCode::E0291,
            "data type is outside this closed private fragment",
            None,
        ),
        R::RecursivePayload { through } => (
            DiagnosticCode::E0291,
            "field leads to a non-finite by-value record payload",
            Some(records.get(&through)?.name),
        ),
        R::NonCopyField { .. } => (
            DiagnosticCode::E0291,
            "copy struct requires Copy evidence for every field",
            None,
        ),
        R::MissingDefinition | R::UnknownSymbol | R::UnknownRecordType { .. } => return None,
    })
}

fn origin(tree: &SyntaxTree, source: SyntaxOrigin) -> Option<SourceOrigin> {
    match source {
        SyntaxOrigin::Token(t) => SourceOrigin::from_token(tree, t).ok(),
        SyntaxOrigin::Node(n) => SourceOrigin::from_node(tree, n).ok(),
    }
}
