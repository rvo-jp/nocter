use std::fmt;

use nocter_syntax::NodeId;

use crate::{
    CompileUnitInput, SourceDiagnostic, SurfaceError,
    diagnostic::{input_trees, origin_from_trees},
};

/// Stable source-level rule for a directory module's authored declaration surface.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SurfaceRule {
    ImplementationVisibility,
    ImplementationMember,
    MissingConstructionVisibility,
    UnknownTargetGate,
}

impl SurfaceRule {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ImplementationVisibility => "E0230",
            Self::ImplementationMember => "E0231",
            Self::MissingConstructionVisibility => "E0232",
            Self::UnknownTargetGate => "E0233",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::ImplementationVisibility => {
                "an implementation source cannot declare non-private visibility"
            }
            Self::ImplementationMember => {
                "this member may be declared only in the module root source"
            }
            Self::MissingConstructionVisibility => {
                "a root-source construction member requires explicit visibility"
            }
            Self::UnknownTargetGate => "target gate names an unrecognized compilation target",
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::ImplementationVisibility => {
                "move the public contract to index.nct or remove the visibility"
            }
            Self::ImplementationMember => "move the declaration to the module's index.nct",
            Self::MissingConstructionVisibility => {
                "add pub, pub(./), or another non-private visibility to the construction member"
            }
            Self::UnknownTargetGate => {
                "use one of the target names recognized by this compiler release"
            }
        }
    }
}

/// One module-surface rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceDiagnostic {
    rule: SurfaceRule,
    source: Box<SourceDiagnostic>,
}

impl SurfaceDiagnostic {
    pub(crate) fn project(
        error: SurfaceError,
        input: &CompileUnitInput<'_>,
    ) -> Result<Self, SurfaceError> {
        let (rule, node) = classify(&error).ok_or_else(|| error.clone())?;
        let primary = origin_from_trees(input_trees(input), node).ok_or(error)?;
        let source = SourceDiagnostic::new(
            rule.code(),
            rule.message(),
            primary,
            Vec::new(),
            Some(rule.help()),
        );
        Ok(Self {
            rule,
            source: Box::new(source),
        })
    }

    #[must_use]
    pub const fn rule(&self) -> SurfaceRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for SurfaceDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for SurfaceDiagnostic {}

const fn classify(error: &SurfaceError) -> Option<(SurfaceRule, NodeId)> {
    match error {
        SurfaceError::ImplementationVisibility(node) => {
            Some((SurfaceRule::ImplementationVisibility, *node))
        }
        SurfaceError::ImplementationMember(node) => {
            Some((SurfaceRule::ImplementationMember, *node))
        }
        SurfaceError::MissingConstructionVisibility(node) => {
            Some((SurfaceRule::MissingConstructionVisibility, *node))
        }
        SurfaceError::UnknownTargetGate(node) => Some((SurfaceRule::UnknownTargetGate, *node)),
        SurfaceError::Topology(_)
        | SurfaceError::SyntaxErrors(_)
        | SurfaceError::InvalidRootShape(_)
        | SurfaceError::InvalidItemShape(_)
        | SurfaceError::InconsistentUseResolution(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    use super::{SurfaceDiagnostic, SurfaceRule};
    use crate::test_support::source_include;
    use crate::{
        CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
        collect_declaration_surface,
    };

    #[test]
    fn implementation_visibility_projects_only_the_visibility_syntax() {
        let (diagnostic, implementation) = implementation_diagnostic(
            "pub func exposed(): void {}\n",
            SurfaceRule::ImplementationVisibility,
        );

        assert_eq!(diagnostic.source().code(), "E0230");
        assert_eq!(diagnostic.source().primary().source(), implementation);
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            0
        );
        assert_eq!(diagnostic.source().primary().span().range().end().get(), 3);
    }

    #[test]
    fn implementation_members_have_a_source_backed_rule() {
        let (diagnostic, implementation) = implementation_diagnostic(
            "struct Hidden { value: usize }\n",
            SurfaceRule::ImplementationMember,
        );

        assert_eq!(diagnostic.source().code(), "E0231");
        assert_eq!(diagnostic.source().primary().source(), implementation);
    }

    #[test]
    fn root_construction_members_require_an_explicit_visibility() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "struct Value { value: usize }\nconstruct Value {\n    func new(): Self { Value { value: 0 } }\n}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = compile_unit(
            &sources,
            &manifest,
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            Vec::new(),
        );

        let error = collect_declaration_surface(&input).unwrap_err();
        let diagnostic = SurfaceDiagnostic::project(error, &input).unwrap();

        assert_eq!(
            diagnostic.rule(),
            SurfaceRule::MissingConstructionVisibility
        );
        assert_eq!(diagnostic.source().code(), "E0232");
        assert_eq!(diagnostic.source().primary().source(), root_id);
    }

    #[test]
    fn unknown_target_names_have_a_source_backed_rule() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "#target: \"unknown-target\"\nfunc main(): void {}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let input = compile_unit(
            &sources,
            &manifest,
            vec![ModuleSourceInput::new(
                "/app/index.nct",
                ModuleSourceKind::Root,
                &root,
            )],
            Vec::new(),
        );

        let error = collect_declaration_surface(&input).unwrap_err();
        let diagnostic = SurfaceDiagnostic::project(error, &input).unwrap();

        assert_eq!(diagnostic.rule(), SurfaceRule::UnknownTargetGate);
        assert_eq!(diagnostic.source().code(), "E0233");
        assert_eq!(diagnostic.source().primary().source(), root_id);
        assert_eq!(
            diagnostic.source().primary().span().range().start().get(),
            9
        );
    }

    fn implementation_diagnostic(
        text: &str,
        expected_rule: SurfaceRule,
    ) -> (SurfaceDiagnostic, nocter_source::SourceId) {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "include ./implementation.nct\n",
        );
        let implementation_id = add_source(&mut sources, "/app/implementation.nct", text);
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
        let input = compile_unit(
            &sources,
            &manifest,
            vec![
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                ModuleSourceInput::new(
                    "/app/implementation.nct",
                    ModuleSourceKind::Implementation,
                    &implementation,
                ),
            ],
            vec![source_include(&root, 0, "/app/implementation.nct")],
        );

        let error = collect_declaration_surface(&input).unwrap_err();
        let diagnostic = SurfaceDiagnostic::project(error, &input).unwrap();
        assert_eq!(diagnostic.rule(), expected_rule);
        (diagnostic, implementation_id)
    }

    fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> nocter_source::SourceId {
        sources
            .add_bytes(SourceName::new(name), text.as_bytes())
            .unwrap()
    }

    fn parse_source(
        sources: &SourceMap,
        source: nocter_source::SourceId,
        goal: ParseGoal,
    ) -> SyntaxTree {
        let tree = parse(sources.get(source).unwrap(), goal);
        assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
        tree
    }

    fn compile_unit<'syntax>(
        sources: &'syntax SourceMap,
        manifest: &'syntax SyntaxTree,
        module_sources: Vec<ModuleSourceInput<'syntax>>,
        resolutions: Vec<crate::IncludeResolutionInput>,
    ) -> CompileUnitInput<'syntax> {
        CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            sources,
            vec![PackageInput::new(
                PackageIdentity::new("workspace:app"),
                "app",
                PackageMode::Declared,
                Some(PackageDeclarationInput::new("/app/nocter.nct", manifest)),
            )],
            vec![ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                module_sources,
            )],
            Vec::new(),
        )
        .with_include_resolutions(resolutions)
    }
}
