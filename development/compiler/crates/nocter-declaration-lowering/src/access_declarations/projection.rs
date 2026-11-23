use super::{
    CodeSource, Failure,
    source::{AliasSource, RecordSource},
};
use nocter_declarations::value_access::{
    DeclarationId, DeclarationRule, DeclarationViolation, HeaderPosition, QualifiedNameError,
};
use nocter_diagnostics::{DiagnosticCode, DiagnosticNote, SourceDiagnostic};
use nocter_model::{AccessCodeId, NominalTypeId, TypeAliasId};
use nocter_source_index::SourceOrigin;
use nocter_syntax::{SyntaxOrigin, SyntaxTree};
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
    project_violations_in(&[tree], sources, records, &BTreeMap::new(), violations)
}
pub(super) fn project_violations_in(
    trees: &[&SyntaxTree],
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    aliases: &BTreeMap<TypeAliasId, AliasSource>,
    violations: Vec<DeclarationViolation>,
) -> Failure {
    let origin = |source| crate::diagnostic::origin_from_syntax(trees.iter().copied(), source);
    let projected = violations
        .into_iter()
        .map(|v| project(&origin, sources, records, aliases, v))
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
    aliases: &BTreeMap<TypeAliasId, AliasSource>,
    violation: DeclarationViolation,
) -> Option<HeaderDiagnostic> {
    use DeclarationRule as R;
    let typed = matches!(
        violation.rule,
        R::UnknownDataName { .. }
            | R::QualifiedDataName { .. }
            | R::UnsupportedDataType { .. }
            | R::UnsupportedCallableData
            | R::NonCopyField
            | R::RepeatedFailure
            | R::RecursivePayload { .. }
            | R::RecursiveAlias
            | R::UnsupportedAliasTarget
    );
    let primary = super::source::header_origin(
        sources,
        records,
        aliases,
        violation.declaration,
        violation.position,
        typed,
    )?;
    let (code, message) = details(violation)?;
    let primary = origin(primary)?;
    let notes = if let Some(related) = related(sources, records, aliases, violation).ok()? {
        Some(DiagnosticNote::new(
            "related declaration is here",
            origin(related)?,
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

fn related(
    sources: &BTreeMap<AccessCodeId, CodeSource>,
    records: &BTreeMap<NominalTypeId, Arc<RecordSource>>,
    aliases: &BTreeMap<TypeAliasId, AliasSource>,
    violation: DeclarationViolation,
) -> Result<Option<SyntaxOrigin>, Failure> {
    use DeclarationRule as R;
    let token = match violation.rule {
        R::DuplicateValue { first } | R::ParameterShadowsValue { declaration: first } => {
            sources.get(&first).and_then(CodeSource::name)
        }
        R::DuplicateRecord { first } | R::RecursivePayload { through: first } => {
            records.get(&first).map(|record| record.name)
        }
        R::DuplicateTypeName { first } => {
            return super::source::header_origin(
                sources,
                records,
                aliases,
                first,
                HeaderPosition::Name,
                false,
            )
            .map(Some)
            .ok_or(Failure::InconsistentSyntax);
        }
        R::DuplicateParameter { first } => {
            return super::source::header_origin(
                sources,
                records,
                aliases,
                violation.declaration,
                first,
                false,
            )
            .map(Some)
            .ok_or(Failure::InconsistentSyntax);
        }
        R::DuplicateField { first } => {
            let DeclarationId::Record(id) = violation.declaration else {
                return Err(Failure::InconsistentSyntax);
            };
            records
                .get(&id)
                .and_then(|record| record.fields.get(first))
                .map(|field| field.name)
        }
        _ => return Ok(None),
    };
    token
        .map(SyntaxOrigin::Token)
        .map(Some)
        .ok_or(Failure::InconsistentSyntax)
}

fn details(violation: DeclarationViolation) -> Option<(DiagnosticCode, &'static str)> {
    use DeclarationRule as R;
    Some(match violation.rule {
        R::InvalidInitializerHeader => (
            DiagnosticCode::E0323,
            "initializer headers require eligible final data and no inputs",
        ),
        R::DuplicateValue { .. } => (DiagnosticCode::E0241, "duplicate value declaration name"),
        R::DuplicateRecord { .. } => (DiagnosticCode::E0241, "duplicate record name"),
        R::DuplicateTypeName { .. } => (DiagnosticCode::E0241, "duplicate type declaration name"),
        R::DuplicateParameter { .. } => (DiagnosticCode::E0341, "duplicate parameter name"),
        R::ParameterPatternMismatch => (
            DiagnosticCode::E0341,
            "parameter pattern does not match the declared tuple data",
        ),
        R::DuplicateField { .. } => (DiagnosticCode::E0241, "duplicate field name"),
        R::ParameterShadowsNamespace { .. } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored namespace binding",
        ),
        R::NameConflictsWithNamespace { .. } => (
            DiagnosticCode::E0241,
            "declaration name conflicts with an existing namespace binding",
        ),
        R::InconsistentBindingAudience { .. } => (
            DiagnosticCode::E0241,
            "the same declaration name has inconsistent audience bindings",
        ),
        R::ParameterShadowsValue { .. } => (
            DiagnosticCode::E0341,
            "parameter shadows an authored value declaration",
        ),
        R::ReservedBuiltinName => (
            if violation.position == HeaderPosition::Name {
                DiagnosticCode::E0240
            } else {
                DiagnosticCode::E0341
            },
            "name is reserved for a built-in type",
        ),
        R::UnknownDataName { .. } => (DiagnosticCode::E0290, "unknown data type"),
        R::QualifiedDataName { error } => qualified_data_details(error),
        R::UnsupportedCallableData => (
            DiagnosticCode::E0291,
            "callable data in this position is outside the qualified witness/type model",
        ),
        R::UnsupportedDataType { .. } => (
            DiagnosticCode::E0291,
            "data type is outside this closed private fragment",
        ),
        R::RecursivePayload { .. } => (
            DiagnosticCode::E0291,
            "field leads to a non-finite by-value record payload",
        ),
        R::RecursiveAlias => (
            DiagnosticCode::E0291,
            "type alias target recursively depends on this alias",
        ),
        R::UnsupportedAliasTarget => (
            DiagnosticCode::E0291,
            "callable and future aliases need qualified type contracts",
        ),
        R::RepeatedFailure => (
            DiagnosticCode::E0291,
            "one uninterrupted outcome chain cannot contain multiple failure layers",
        ),
        R::NonCopyField => (
            DiagnosticCode::E0291,
            "copy struct requires Copy evidence for every field",
        ),
        R::MissingDefinition
        | R::UnknownSymbol
        | R::UnknownRecordType { .. }
        | R::UnreadyAlias { .. } => return None,
    })
}

fn qualified_data_details(error: QualifiedNameError) -> (DiagnosticCode, &'static str) {
    match error {
        QualifiedNameError::Inaccessible { .. } => (
            DiagnosticCode::E0412,
            "qualified data type is outside its visibility boundary",
        ),
        _ => (
            DiagnosticCode::E0290,
            "qualified data path does not name an accessible type",
        ),
    }
}
