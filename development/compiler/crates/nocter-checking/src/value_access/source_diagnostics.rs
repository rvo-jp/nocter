//! One source-failure report for all downstream consumers; no invented diagnostic locations.
use super::{Failure as BodyFailure, accepted, staged::SourceFailure};
use nocter_compile_input::CompileUnitInput;
use nocter_diagnostics::{DiagnosticOrigin, SourceDiagnostic};
use nocter_source_index::SourceOrigin;
use nocter_syntax::SyntaxOrigin;

#[derive(Debug)]
pub struct SourceFailureReport {
    failure: SourceFailure,
    diagnostics: Box<[SourceDiagnostic]>,
}
impl SourceFailureReport {
    #[must_use]
    pub const fn failure(&self) -> &SourceFailure {
        &self.failure
    }
    #[must_use]
    pub const fn diagnostics(&self) -> &[SourceDiagnostic] {
        &self.diagnostics
    }
}

impl std::fmt::Display for SourceFailureReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.diagnostics.first() {
            Some(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            None => formatter.write_str("value-access source analysis failed"),
        }
    }
}
impl std::error::Error for SourceFailureReport {}

impl SourceFailure {
    /// Retains the original typed failure and only diagnostics established by its owning phase.
    /// An empty diagnostic set does not turn an internal/metadata failure into authored rejection.
    /// # Errors
    /// Returns the original failure if a selected diagnostic does not belong to this exact input.
    pub fn project(self, input: &CompileUnitInput<'_>) -> Result<SourceFailureReport, Self> {
        let diagnostics = match &self {
            Self::Declarations(failure) => {
                Some(failure.source_diagnostics().into_iter().cloned().collect())
            }
            Self::Modules(failure) => failure.project_diagnostics(input),
            Self::Checking(failure) => Some(match failure {
                accepted::Failure::Body {
                    failure: BodyFailure::Rejected(diagnostic),
                    ..
                } => vec![diagnostic.source().clone()].into_boxed_slice(),
                accepted::Failure::Flow { diagnostics, .. } => diagnostics.clone(),
                accepted::Failure::Evaluation { diagnostic, .. } => {
                    vec![diagnostic.source().clone()].into_boxed_slice()
                }
                accepted::Failure::Const { diagnostic, .. } => {
                    vec![diagnostic.source().clone()].into_boxed_slice()
                }
                accepted::Failure::Body { .. } | accepted::Failure::Integrity(_) => Box::new([]),
            }),
        };
        let Some(diagnostics) = diagnostics else {
            return Err(self);
        };
        if diagnostics.iter().any(|diagnostic| {
            !matches_input(diagnostic.primary(), input)
                || diagnostic
                    .notes()
                    .iter()
                    .any(|note| !matches_input(note.origin(), input))
        }) {
            return Err(self);
        }
        Ok(SourceFailureReport {
            failure: self,
            diagnostics,
        })
    }
}

fn matches_input(origin: DiagnosticOrigin, input: &CompileUnitInput<'_>) -> bool {
    let Some(source) = input.sources().get(origin.source()) else {
        return false;
    };
    if source.text_at(origin.span().range()).is_none() {
        return false;
    }
    match origin {
        DiagnosticOrigin::Span(_) => input
            .modules()
            .iter()
            .flat_map(nocter_compile_input::ModuleInput::sources)
            .any(|selected| selected.syntax().source() == origin.source()),
        DiagnosticOrigin::Syntax(origin) => input
            .modules()
            .iter()
            .flat_map(nocter_compile_input::ModuleInput::sources)
            .any(|source| {
                let tree = source.syntax();
                let projected = match origin.syntax() {
                    SyntaxOrigin::Node(node) => SourceOrigin::from_node(tree, node).ok(),
                    SyntaxOrigin::Token(token) => SourceOrigin::from_token(tree, token).ok(),
                };
                projected == Some(origin)
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_access::{
        accepted::SourceUnit,
        tests::{bound, input},
    };
    use nocter_compile_input::{ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind};
    use nocter_model::{CompilationTarget, PackageIdentity};
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    fn selected<'a>(sources: &'a SourceMap, tree: &'a SyntaxTree) -> CompileUnitInput<'a> {
        CompileUnitInput::for_goal(
            ParseGoal::V2CoreUnit,
            CompilationTarget::Arm64Darwin,
            sources,
            Vec::new(),
            vec![ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("single:app"), Vec::<&str>::new()),
                vec![ModuleSourceInput::new(
                    "body.nct",
                    ModuleSourceKind::SingleFile,
                    tree,
                )],
            )],
            Vec::new(),
        )
    }
    fn failure(sources: &SourceMap, tree: &SyntaxTree) -> SourceFailure {
        let Err(failure) = SourceUnit::prepare(bound(sources, tree)) else {
            panic!("invalid source")
        };
        failure
    }

    #[test]
    fn reports_original_header_body_flow_and_required_execution_diagnostics() {
        for text in [
            "func run(value: Missing): i32 { 0 }",
            "func run(): i32 { missing }",
            "func run(): i32 { var value = 1\nread held = value\nvalue = 2\nheld }",
            "const RESULT: i32 = 1 / 0",
            "func runtime(): i32 { 42 }\nconst func bad(): i32 { runtime() }",
            "primitive func unselected(): i32",
            "func missing_body(): i32",
        ] {
            let (sources, tree) = input(text);
            let report = failure(&sources, &tree)
                .project(&selected(&sources, &tree))
                .unwrap();
            assert!(!report.diagnostics().is_empty(), "{text}");
            for diagnostic in report.diagnostics() {
                assert_eq!(diagnostic.primary().source(), tree.source());
                assert!(
                    sources
                        .get(tree.source())
                        .unwrap()
                        .text_at(diagnostic.primary().span().range())
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn rejects_foreign_trees_and_source_maps_instead_of_relabeling_diagnostics() {
        let (sources, tree) = input("func run(): i32 {}");
        let report = failure(&sources, &tree)
            .project(&selected(&sources, &tree))
            .unwrap();
        assert!(report.diagnostics()[0].primary().node().is_some());
        let reparsed = parse(sources.get(tree.source()).unwrap(), ParseGoal::V2CoreUnit);
        assert!(
            failure(&sources, &tree)
                .project(&selected(&sources, &reparsed))
                .is_err()
        );
        let mut foreign = SourceMap::new();
        foreign
            .add_bytes(SourceName::new("body.nct"), b"func run(): i32 {}")
            .unwrap();
        assert!(
            failure(&sources, &tree)
                .project(&selected(&foreign, &tree))
                .is_err()
        );
    }

    #[test]
    fn lexical_tokens_survive_equal_source_reparsing_but_unselected_spans_and_notes_do_not() {
        use nocter_diagnostics::{DiagnosticCode, DiagnosticNote};
        use nocter_source::{ByteOffset, TextRange};
        let (mut sources, tree) = input("func run(): i32 { missing }");
        let reparsed = parse(sources.get(tree.source()).unwrap(), ParseGoal::V2CoreUnit);
        let report = failure(&sources, &tree)
            .project(&selected(&sources, &reparsed))
            .unwrap();
        assert!(report.diagnostics()[0].primary().token().is_some());
        let extra = sources
            .add_bytes(SourceName::new("unselected.nct"), b"bad")
            .unwrap();
        let span = sources
            .get(extra)
            .unwrap()
            .span(TextRange::new(ByteOffset::new(0), ByteOffset::new(1)));
        let primary = SourceOrigin::from_node(&tree, tree.root_id()).unwrap();
        for diagnostic in [
            SourceDiagnostic::new(DiagnosticCode::E0120, "unselected", span, [], None::<&str>),
            SourceDiagnostic::new(
                DiagnosticCode::E0120,
                "selected",
                primary,
                [DiagnosticNote::new("unselected note", span)],
                None::<&str>,
            ),
        ] {
            let failure = SourceFailure::Declarations(
                nocter_declaration_lowering::access_declarations::Failure::Syntax(
                    vec![diagnostic].into(),
                ),
            );
            assert!(failure.project(&selected(&sources, &tree)).is_err());
        }
    }

    #[test]
    fn metadata_and_integrity_failures_receive_no_fabricated_source_diagnostic() {
        let (sources, tree) = input("func run(): i32 { 42 }");
        for failure in [
            SourceFailure::Declarations(
                nocter_declaration_lowering::access_declarations::Failure::InconsistentSyntax,
            ),
            SourceFailure::Modules(
                nocter_declaration_lowering::access_declarations::ModuleSourcesFailure::Toolchain(
                    nocter_declaration_lowering::ToolchainError::MissingProfile,
                ),
            ),
        ] {
            let report = failure.project(&selected(&sources, &tree)).unwrap();
            assert!(report.diagnostics().is_empty());
        }
    }
}
