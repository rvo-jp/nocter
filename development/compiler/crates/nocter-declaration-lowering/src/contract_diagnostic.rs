use std::fmt;

use nocter_source_index::SourceOrigin;
use nocter_syntax::NodeId;

use crate::{
    DeclarationContractError, DeclarationSurface, DiagnosticNote, SourceDiagnostic,
    diagnostic::origin_from_trees,
};

/// Stable source-level rule for public contract and private body joining.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeclarationContractRule {
    MissingBody,
    MismatchedBody,
    DuplicateBody,
    InvalidBodyOmission,
    UncontractedConformance,
    UncontractedInterfaceDefault,
    MissingRepresentation,
    MismatchedRepresentation,
    DuplicateRepresentation,
    RepresentationCompletedAgain,
}

impl DeclarationContractRule {
    pub const ALL: [Self; 10] = [
        Self::MissingBody,
        Self::MismatchedBody,
        Self::DuplicateBody,
        Self::InvalidBodyOmission,
        Self::UncontractedConformance,
        Self::UncontractedInterfaceDefault,
        Self::MissingRepresentation,
        Self::MismatchedRepresentation,
        Self::DuplicateRepresentation,
        Self::RepresentationCompletedAgain,
    ];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MissingBody => "E0250",
            Self::MismatchedBody => "E0251",
            Self::DuplicateBody => "E0252",
            Self::InvalidBodyOmission => "E0253",
            Self::UncontractedConformance => "E0254",
            Self::UncontractedInterfaceDefault => "E0259",
            Self::MissingRepresentation => "E0255",
            Self::MismatchedRepresentation => "E0256",
            Self::DuplicateRepresentation => "E0257",
            Self::RepresentationCompletedAgain => "E0258",
        }
    }

    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::MissingBody => "public callable contract has no implementation body",
            Self::MismatchedBody => {
                "private implementation body does not match its public contract"
            }
            Self::DuplicateBody => "public callable contract has more than one implementation body",
            Self::InvalidBodyOmission => {
                "callable omits its body outside an eligible public contract"
            }
            Self::UncontractedConformance => {
                "implementation conformance has no public index contract"
            }
            Self::UncontractedInterfaceDefault => {
                "interface default implementation has no public index contract"
            }
            Self::MissingRepresentation => {
                "public nominal contract has no private representation definition"
            }
            Self::MismatchedRepresentation => {
                "private nominal representation does not match its public contract"
            }
            Self::DuplicateRepresentation => {
                "public nominal contract has more than one private representation"
            }
            Self::RepresentationCompletedAgain => {
                "a nominal representation is used to complete more than one contract"
            }
        }
    }

    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::MissingBody => "add one exact private implementation body in the same module",
            Self::MismatchedBody => {
                "make the private body header exactly match the public contract"
            }
            Self::DuplicateBody => "keep exactly one matching private implementation body",
            Self::InvalidBodyOmission => {
                "write the body inline or declare an eligible public root contract"
            }
            Self::UncontractedConformance => {
                "declare the conformance contract and every implemented method in index.nct"
            }
            Self::UncontractedInterfaceDefault => {
                "declare the default method contract in the reciprocally included index.nct interface"
            }
            Self::MissingRepresentation => {
                "add one reciprocal directly included private representation"
            }
            Self::MismatchedRepresentation => {
                "make the representation kind, name, modifiers, and generic header match"
            }
            Self::DuplicateRepresentation => "keep exactly one matching private representation",
            Self::RepresentationCompletedAgain => {
                "give each public nominal contract one distinct representation"
            }
        }
    }

    #[must_use]
    pub const fn related_message(self) -> Option<&'static str> {
        match self {
            Self::MismatchedBody
            | Self::DuplicateBody
            | Self::MismatchedRepresentation
            | Self::DuplicateRepresentation
            | Self::RepresentationCompletedAgain => Some("public contract is declared here"),
            Self::MissingBody
            | Self::InvalidBodyOmission
            | Self::UncontractedConformance
            | Self::UncontractedInterfaceDefault
            | Self::MissingRepresentation => None,
        }
    }
}

/// A callable-contract rule projected to exact source syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationContractDiagnostic {
    rule: DeclarationContractRule,
    source: Box<SourceDiagnostic>,
}

impl DeclarationContractDiagnostic {
    pub(crate) fn project(
        error: DeclarationContractError,
        surface: &DeclarationSurface<'_>,
    ) -> Result<Self, DeclarationContractError> {
        let rule = rule(error).ok_or(error)?;
        let primary = origin(surface, primary_node(error)).ok_or(error)?;
        let notes = related_node(error)
            .zip(rule.related_message())
            .map(|(node, message)| {
                origin(surface, node)
                    .map(|origin| DiagnosticNote::new(message, origin))
                    .ok_or(error)
            })
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
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
    pub const fn rule(&self) -> DeclarationContractRule {
        self.rule
    }

    #[must_use]
    pub const fn source(&self) -> &SourceDiagnostic {
        &self.source
    }
}

impl fmt::Display for DeclarationContractDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}",
            self.source.code(),
            self.source.message()
        )
    }
}

impl std::error::Error for DeclarationContractDiagnostic {}

const fn rule(error: DeclarationContractError) -> Option<DeclarationContractRule> {
    match error {
        DeclarationContractError::MissingBody(_) => Some(DeclarationContractRule::MissingBody),
        DeclarationContractError::MismatchedBody { .. } => {
            Some(DeclarationContractRule::MismatchedBody)
        }
        DeclarationContractError::DuplicateBody { .. } => {
            Some(DeclarationContractRule::DuplicateBody)
        }
        DeclarationContractError::InvalidBodyOmission(_) => {
            Some(DeclarationContractRule::InvalidBodyOmission)
        }
        DeclarationContractError::UncontractedConformance(_) => {
            Some(DeclarationContractRule::UncontractedConformance)
        }
        DeclarationContractError::UncontractedInterfaceDefault(_) => {
            Some(DeclarationContractRule::UncontractedInterfaceDefault)
        }
        DeclarationContractError::MissingRepresentation(_) => {
            Some(DeclarationContractRule::MissingRepresentation)
        }
        DeclarationContractError::MismatchedRepresentation { .. } => {
            Some(DeclarationContractRule::MismatchedRepresentation)
        }
        DeclarationContractError::DuplicateRepresentation { .. } => {
            Some(DeclarationContractRule::DuplicateRepresentation)
        }
        DeclarationContractError::RepresentationCompletedAgain { .. } => {
            Some(DeclarationContractRule::RepresentationCompletedAgain)
        }
        DeclarationContractError::InconsistentSurface(_) => None,
    }
}

const fn primary_node(error: DeclarationContractError) -> NodeId {
    match error {
        DeclarationContractError::MissingBody(node)
        | DeclarationContractError::InvalidBodyOmission(node)
        | DeclarationContractError::UncontractedConformance(node)
        | DeclarationContractError::UncontractedInterfaceDefault(node)
        | DeclarationContractError::MissingRepresentation(node)
        | DeclarationContractError::InconsistentSurface(node) => node,
        DeclarationContractError::MismatchedBody { body, .. }
        | DeclarationContractError::DuplicateBody { body, .. } => body,
        DeclarationContractError::MismatchedRepresentation { definition, .. }
        | DeclarationContractError::DuplicateRepresentation { definition, .. }
        | DeclarationContractError::RepresentationCompletedAgain { definition, .. } => definition,
    }
}

const fn related_node(error: DeclarationContractError) -> Option<NodeId> {
    match error {
        DeclarationContractError::MismatchedBody { contract, .. }
        | DeclarationContractError::DuplicateBody { contract, .. }
        | DeclarationContractError::MismatchedRepresentation { contract, .. }
        | DeclarationContractError::DuplicateRepresentation { contract, .. }
        | DeclarationContractError::RepresentationCompletedAgain { contract, .. } => Some(contract),
        DeclarationContractError::MissingBody(_)
        | DeclarationContractError::InvalidBodyOmission(_)
        | DeclarationContractError::UncontractedConformance(_)
        | DeclarationContractError::UncontractedInterfaceDefault(_)
        | DeclarationContractError::MissingRepresentation(_)
        | DeclarationContractError::InconsistentSurface(_) => None,
    }
}

fn origin(surface: &DeclarationSurface<'_>, node: NodeId) -> Option<SourceOrigin> {
    origin_from_trees(
        surface.sources().iter().map(crate::SurfaceSource::syntax),
        node,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use nocter_source::{SourceMap, SourceName};
    use nocter_syntax::{ParseGoal, SyntaxTree, parse};

    use super::{DeclarationContractDiagnostic, DeclarationContractRule};
    use crate::test_support::source_include;
    use crate::{
        CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
        PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
        analyze_declaration_contracts, collect_declaration_surface,
    };

    #[test]
    fn declaration_contract_rule_codes_are_closed_and_unique() {
        let codes: BTreeSet<_> = DeclarationContractRule::ALL
            .into_iter()
            .map(DeclarationContractRule::code)
            .collect();
        assert_eq!(codes.len(), DeclarationContractRule::ALL.len());
    }

    #[test]
    fn mismatch_projects_body_as_primary_and_contract_as_related() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "include ./parse.nct\n\npub func parse(text: &str): usize\n",
        );
        let implementation_id = add_source(
            &mut sources,
            "/app/parse.nct",
            "include ./index.nct\n\nfunc parse(text: usize): usize { text }\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
        let package = PackageInput::new(
            PackageIdentity::new("workspace:app"),
            "app",
            PackageMode::Declared,
            Some(PackageDeclarationInput::new("/app/nocter.nct", &manifest)),
        );
        let module = ModuleInput::new(
            ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
            vec![
                ModuleSourceInput::new(
                    "/app/parse.nct",
                    ModuleSourceKind::Implementation,
                    &implementation,
                ),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ],
        );
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![package],
            vec![module],
            Vec::new(),
        )
        .with_include_resolutions(vec![
            source_include(&root, 0, "/app/parse.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ]);
        let surface = collect_declaration_surface(&input).unwrap();

        let error = analyze_declaration_contracts(&surface).unwrap_err();
        let diagnostic = DeclarationContractDiagnostic::project(error, &surface).unwrap();

        assert_eq!(diagnostic.rule(), DeclarationContractRule::MismatchedBody);
        assert_eq!(diagnostic.source().code(), "E0251");
        assert_eq!(diagnostic.source().primary().source(), implementation_id);
        assert_eq!(diagnostic.source().notes().len(), 1);
        assert_eq!(diagnostic.source().notes()[0].origin().source(), root_id);
        assert_eq!(
            diagnostic.source().notes()[0].message(),
            "public contract is declared here"
        );
    }

    #[test]
    fn implementation_only_conformance_projects_its_authored_source() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            concat!(
                "include ./value.nct\n",
                "pub interface Read { pub method &self.read(): usize }\n",
                "pub struct Value {}\n",
            ),
        );
        let implementation_id = add_source(
            &mut sources,
            "/app/value.nct",
            concat!(
                "include ./index.nct\n",
                "conform Read for Value {\n",
                "    method &self.read(): usize { return 0 }\n",
                "}\n",
            ),
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![PackageInput::new(
                PackageIdentity::new("workspace:app"),
                "app",
                PackageMode::Declared,
                Some(PackageDeclarationInput::new("/app/nocter.nct", &manifest)),
            )],
            vec![ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                vec![
                    ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                    ModuleSourceInput::new(
                        "/app/value.nct",
                        ModuleSourceKind::Implementation,
                        &implementation,
                    ),
                ],
            )],
            Vec::new(),
        )
        .with_include_resolutions(vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ]);
        let surface = collect_declaration_surface(&input).unwrap();
        let error = analyze_declaration_contracts(&surface).unwrap_err();
        let diagnostic = DeclarationContractDiagnostic::project(error, &surface).unwrap();

        assert_eq!(
            diagnostic.rule(),
            DeclarationContractRule::UncontractedConformance
        );
        assert_eq!(diagnostic.source().code(), "E0254");
        assert_eq!(diagnostic.source().primary().source(), implementation_id);
    }

    #[test]
    fn private_implementation_entries_do_not_require_public_contracts() {
        let mut sources = SourceMap::new();
        let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let root_id = add_source(
            &mut sources,
            "/app/index.nct",
            "include ./build.nct\n\nstruct Value {}\n",
        );
        let implementation_id = add_source(
            &mut sources,
            "/app/build.nct",
            "include ./index.nct\n\nconstruct Value {\n    func new(): Self {}\n}\n",
        );
        let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
        let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
        let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
        let input = CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &sources,
            vec![PackageInput::new(
                PackageIdentity::new("workspace:app"),
                "app",
                PackageMode::Declared,
                Some(PackageDeclarationInput::new("/app/nocter.nct", &manifest)),
            )],
            vec![ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                vec![
                    ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
                    ModuleSourceInput::new(
                        "/app/build.nct",
                        ModuleSourceKind::Implementation,
                        &implementation,
                    ),
                ],
            )],
            Vec::new(),
        )
        .with_include_resolutions(vec![
            source_include(&root, 0, "/app/build.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ]);
        let surface = collect_declaration_surface(&input).unwrap();
        analyze_declaration_contracts(&surface).unwrap();
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
}
