use super::{CodeSource, Failure, source::RecordSource};
use nocter_declarations::value_access::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition,
};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessCodeId, NominalTypeId};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{SyntaxOrigin, SyntaxToken, SyntaxTree};
use std::{collections::BTreeMap, sync::Arc};

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
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    violations: Vec<DeclarationViolation>,
) -> Failure {
    project_violations_in(&[tree], sources, records, violations)
}
pub(super) fn project_violations_in(
    trees: &[&SyntaxTree],
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    violations: Vec<DeclarationViolation>,
) -> Failure {
    let origin = |source| crate::diagnostic::origin_from_syntax(trees.iter().copied(), source);
    let projected = violations
        .into_iter()
        .map(|v| project(&origin, sources, records, v))
        .collect::<Option<Vec<_>>>();
    let Some(mut diagnostics) = projected else {
        return Failure::InconsistentSyntax;
    };
    let order: BTreeMap<_, _> = trees
        .iter()
        .enumerate()
        .map(|(index, tree)| (tree.source(), index))
        .collect();
    diagnostics.sort_by_key(|d| {
        (
            order[&d.source.primary().source()],
            d.source.primary().span().range().start(),
        )
    });
    Failure::Headers(diagnostics)
}
fn project(
    origin: &impl Fn(SyntaxOrigin) -> Option<SourceOrigin>,
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    violation: DeclarationViolation,
) -> Option<HeaderDiagnostic> {
    use DeclarationRule as R;
    let typed = matches!(
        violation.rule,
        R::UnknownDataName { .. }
            | R::UnsupportedDataType { .. }
            | R::NonCopyField
            | R::RepeatedFailure
            | R::RecursivePayload { .. }
    );
    let primary = super::source::header_origin(
        sources,
        records,
        violation.declaration,
        violation.position,
        typed,
    )?;
    let (code, message, related) = details(sources, records, violation)?;
    let primary = origin(primary)?;
    let notes = if let Some(token) = related {
        Some(DiagnosticNote::new(
            "related declaration is here",
            origin(SyntaxOrigin::Token(token))?,
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

fn details(
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    violation: DeclarationViolation,
) -> Option<(DiagnosticCode, &'static str, Option<SyntaxToken>)> {
    use DeclarationRule as R;
    Some(match violation.rule {
        R::InvalidInitializerHeader => (
            DiagnosticCode::E0323,
            "initializer headers require a data value and no inputs",
            None,
        ),
        R::DuplicateValue { first } => (
            DiagnosticCode::E0241,
            "duplicate value declaration name",
            Some(sources.get(&first)?.name()?),
        ),
        R::DuplicateRecord { first } => (
            DiagnosticCode::E0241,
            "duplicate record name",
            Some(records.get(&first)?.name),
        ),
        R::DuplicateParameter { first } => {
            let DeclarationId::Code(id) = violation.declaration else {
                return None;
            };
            (
                DiagnosticCode::E0341,
                "duplicate parameter name",
                Some(*sources.get(&id)?.parameters().get(first)?),
            )
        }
        R::DuplicateField { first } => {
            let DeclarationId::Record(id) = violation.declaration else {
                return None;
            };
            (
                DiagnosticCode::E0241,
                "duplicate field name",
                Some(records.get(&id)?.fields.get(first)?.name),
            )
        }
        R::ParameterShadowsNamespace { .. } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored namespace binding",
            None,
        ),
        R::NameConflictsWithNamespace { .. } => (
            DiagnosticCode::E0241,
            "declaration name conflicts with an existing namespace binding",
            None,
        ),
        R::InconsistentBindingAudience { .. } => (
            DiagnosticCode::E0241,
            "the same declaration name has inconsistent audience bindings",
            None,
        ),
        R::ParameterShadowsValue { declaration } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored value declaration",
            Some(sources.get(&declaration)?.name()?),
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
        R::RepeatedFailure => (
            DiagnosticCode::E0291,
            "one uninterrupted outcome chain cannot contain multiple failure layers",
            None,
        ),
        R::NonCopyField => (
            DiagnosticCode::E0291,
            "copy struct requires Copy evidence for every field",
            None,
        ),
        R::MissingDefinition | R::UnknownSymbol | R::UnknownRecordType { .. } => return None,
    })
}
