use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageInput, PackageMode, StandardRoleInput, ToolchainInput,
    UseResolutionInput, UseTargetInput,
};
use nocter_model::PackageIdentity;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

pub(crate) struct Fixture {
    sources: SourceMap,
    app_manifest: SyntaxTree,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    child: Option<SyntaxTree>,
    standard: SyntaxTree,
    prelude: SyntaxTree,
}

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

impl Fixture {
    pub(crate) fn new(app: &str) -> Self {
        Self::with_standard(app, "")
    }

    pub(crate) fn with_standard(app: &str, standard: &str) -> Self {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", app);
        let std_id = add_source(&mut sources, "/std/index.nct", standard);
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        Self {
            app_manifest: parsed(&sources, app_manifest_id, ParseGoal::PackageFile),
            std_manifest: parsed(&sources, std_manifest_id, ParseGoal::PackageFile),
            app: parsed(&sources, app_id, ParseGoal::SourceFile),
            child: None,
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
            package(
                "workspace:app",
                "app",
                "/app/nocter.nct",
                &self.app_manifest,
            ),
            package(
                "toolchain:std",
                "std",
                "/std/nocter.nct",
                &self.std_manifest,
            ),
        ];
        let mut modules = vec![
            module("workspace:app", &[], "/app/index.nct", &self.app),
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
                UseTargetInput::Module(ModuleIdentity::new(
                    PackageIdentity::new("workspace:app"),
                    ["child"],
                )),
            )]
        });
        CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            resolutions,
        )
        .with_toolchain(ToolchainInput::new(
            PackageIdentity::new("toolchain:std"),
            prelude,
            Vec::new(),
            Vec::new(),
        ))
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
        if tree
            .node(node)
            .is_some_and(|node| node.kind() == NodeKind::UseDeclaration)
        {
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

fn package<'syntax>(
    identity: &str,
    name: &str,
    path: &str,
    manifest: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        PackageIdentity::new(identity),
        name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, manifest)),
    )
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
