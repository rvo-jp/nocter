use nocter_compile_input::{
    BuiltinTypeInput, CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageInput, PackageMode, SourceVisibilityResolutionInput,
    StandardRoleInput, ToolchainInput, UseResolutionInput,
};
use nocter_model::{BuiltinType, PackageIdentity};
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

pub(crate) struct Fixture {
    sources: SourceMap,
    app: SyntaxTree,
    child: Option<SyntaxTree>,
    implementations: Vec<(Box<str>, SyntaxTree)>,
    standard: SyntaxTree,
    prelude: SyntaxTree,
}

pub(crate) const BUILTIN_DECLARATIONS: &str = "\
pub primitive type bool\n\
pub primitive type i8\n\
pub primitive type i16\n\
pub primitive type i32\n\
pub primitive type i64\n\
pub primitive type u8\n\
pub primitive type u16\n\
pub primitive type u32\n\
pub primitive type u64\n\
pub primitive type usize\n\
pub primitive type isize\n\
pub primitive type str\n\
pub primitive type error\n\
pub primitive type void\n\
pub primitive type never\n";

pub(crate) fn with_standard_roles(
    input: CompileUnitInput<'_>,
    roles: Vec<StandardRoleInput>,
) -> CompileUnitInput<'_> {
    let toolchain = input
        .toolchain()
        .expect("checking fixture always supplies a toolchain profile")
        .clone()
        .with_standard_roles(roles);
    input.with_toolchain(toolchain)
}

pub(crate) fn builtin_toolchain(
    sources: &SourceMap,
    standard: &SyntaxTree,
    prelude: ModuleIdentity,
) -> ToolchainInput {
    ToolchainInput::new(
        PackageIdentity::new("toolchain:std"),
        prelude,
        Vec::new(),
        Vec::new(),
    )
    .with_builtin_types(
        BuiltinType::ALL
            .iter()
            .copied()
            .map(|builtin| {
                BuiltinTypeInput::new(
                    builtin,
                    declaration_token(
                        sources,
                        standard,
                        NodeKind::PrimitiveTypeDeclaration,
                        builtin.spelling(),
                    ),
                )
            })
            .collect(),
    )
}

impl Fixture {
    pub(crate) fn new(app: &str) -> Self {
        Self::with_standard(app, "")
    }

    pub(crate) fn with_standard(app: &str, standard: &str) -> Self {
        let mut sources = SourceMap::new();
        let app_id = add_source(&mut sources, "/app/index.nct", app);
        let standard = format!("{BUILTIN_DECLARATIONS}{standard}");
        let std_id = add_source(&mut sources, "/std/index.nct", &standard);
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        Self {
            app: parsed(&sources, app_id, ParseGoal::SourceFile),
            child: None,
            implementations: Vec::new(),
            standard: parsed(&sources, std_id, ParseGoal::SourceFile),
            prelude: parsed(&sources, prelude_id, ParseGoal::SourceFile),
            sources,
        }
    }

    pub(crate) fn with_child(app: &str, child: &str) -> Self {
        let mut fixture = Self::new(app);
        let child_id = add_source(&mut fixture.sources, "/app/child/index.nct", child);
        fixture.child = Some(parsed(&fixture.sources, child_id, ParseGoal::SourceFile));
        fixture
    }

    pub(crate) fn with_implementation_sources(app: &str, implementations: &[(&str, &str)]) -> Self {
        let mut fixture = Self::new(app);
        for (name, text) in implementations {
            let path: Box<str> = format!("/app/{name}").into();
            let source = add_source(&mut fixture.sources, &path, text);
            fixture.implementations.push((
                path,
                parsed(&fixture.sources, source, ParseGoal::SourceFile),
            ));
        }
        fixture
    }

    pub(crate) fn standard_declaration_token(
        &self,
        kind: NodeKind,
        name: &str,
    ) -> nocter_syntax::SyntaxToken {
        declaration_token(&self.sources, &self.standard, kind, name)
    }

    pub(crate) fn app_declaration_token(
        &self,
        kind: NodeKind,
        name: &str,
    ) -> nocter_syntax::SyntaxToken {
        declaration_token(&self.sources, &self.app, kind, name)
    }

    pub(crate) fn input(&self, reverse: bool) -> CompileUnitInput<'_> {
        let mut packages = vec![
            package("workspace:app", "app"),
            package("toolchain:std", "std"),
        ];
        let mut app_sources = vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &self.app,
        )];
        app_sources.extend(self.implementations.iter().map(|(path, syntax)| {
            ModuleSourceInput::new(path.as_ref(), ModuleSourceKind::Implementation, syntax)
        }));
        let mut modules = vec![
            ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                app_sources,
            ),
            module("toolchain:std", &[], "/std/index.nct", &self.standard),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &self.prelude,
            ),
        ];
        if let Some(child) = &self.child {
            modules.push(module(
                "workspace:app",
                &["child"],
                "/app/child/index.nct",
                child,
            ));
        }
        if reverse {
            packages.reverse();
            modules.reverse();
        }
        let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        let resolutions = self.child.as_ref().map_or_else(Vec::new, |_| {
            vec![UseResolutionInput::new(
                use_declaration(&self.app),
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["child"]),
            )]
        });
        CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            resolutions,
        )
        .with_source_visibility_resolutions(self.source_visibility_resolutions())
        .with_toolchain(builtin_toolchain(&self.sources, &self.standard, prelude))
    }

    fn source_visibility_resolutions(&self) -> Vec<SourceVisibilityResolutionInput> {
        std::iter::once(&self.app)
            .chain(self.implementations.iter().map(|(_, syntax)| syntax))
            .flat_map(|tree| {
                tree.children(tree.root_id()).iter().filter_map(|element| {
                    let SyntaxElement::Node(declaration) = element else {
                        return None;
                    };
                    if tree.node(*declaration).map(nocter_syntax::SyntaxNode::kind)
                        != Some(NodeKind::SourceVisibilityDeclaration)
                    {
                        return None;
                    }
                    let path = tree.children(*declaration).iter().find_map(|element| {
                        let SyntaxElement::Node(path) = element else {
                            return None;
                        };
                        (tree.node(*path).map(nocter_syntax::SyntaxNode::kind)
                            == Some(NodeKind::SourceVisibilityPath))
                        .then_some(*path)
                    })?;
                    let authored = self
                        .sources
                        .get(tree.source())?
                        .text_at(tree.node(path)?.range())?;
                    let relative = authored.strip_prefix("./")?;
                    Some(SourceVisibilityResolutionInput::new(
                        *declaration,
                        format!("/app/{relative}"),
                    ))
                })
            })
            .collect()
    }
}

fn declaration_token(
    sources: &SourceMap,
    tree: &SyntaxTree,
    kind: NodeKind,
    name: &str,
) -> nocter_syntax::SyntaxToken {
    let source = sources.get(tree.source()).unwrap();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            let mut descendants = vec![node];
            while let Some(descendant) = descendants.pop() {
                for child in tree.children(descendant).iter().rev() {
                    match child {
                        SyntaxElement::Token(token)
                            if source
                                .text_at(token.range())
                                .is_some_and(|text| text == name) =>
                        {
                            return *token;
                        }
                        SyntaxElement::Node(child) => descendants.push(*child),
                        SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
                    }
                }
            }
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    panic!("fixture declaration {kind:?} named {name:?} does not exist");
}

fn use_declaration(tree: &SyntaxTree) -> nocter_syntax::NodeId {
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| {
            matches!(
                node.kind(),
                NodeKind::UseDeclaration | NodeKind::BlockUseDeclaration
            )
        }) {
            return node;
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    panic!("child-module fixture requires one module use declaration");
}

fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> SourceId {
    sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap()
}

fn parsed(sources: &SourceMap, source: SourceId, goal: ParseGoal) -> SyntaxTree {
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
    tree
}

fn package(identity: &str, name: &str) -> PackageInput {
    PackageInput::new(PackageIdentity::new(identity), name, PackageMode::Declared)
}

fn module<'syntax>(
    identity: &str,
    path: &[&str],
    source_path: &str,
    source: &'syntax SyntaxTree,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(identity), path.iter().copied()),
        vec![ModuleSourceInput::new(
            source_path,
            ModuleSourceKind::Root,
            source,
        )],
    )
}
