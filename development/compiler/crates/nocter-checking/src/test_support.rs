use nocter_declaration_lowering::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
};
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

pub(crate) struct Fixture {
    sources: SourceMap,
    app_manifest: SyntaxTree,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    standard: SyntaxTree,
    prelude: SyntaxTree,
}

impl Fixture {
    pub(crate) fn new(app: &str) -> Self {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", app);
        let std_id = add_source(&mut sources, "/std/index.nct", "");
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        Self {
            app_manifest: parsed(&sources, app_manifest_id, ParseGoal::PackageFile),
            std_manifest: parsed(&sources, std_manifest_id, ParseGoal::PackageFile),
            app: parsed(&sources, app_id, ParseGoal::ModuleSource),
            standard: parsed(&sources, std_id, ParseGoal::ModuleSource),
            prelude: parsed(&sources, prelude_id, ParseGoal::ModuleSource),
            sources,
        }
    }

    pub(crate) fn input(&self, reverse: bool) -> (CompileUnitInput<'_>, ModuleIdentity) {
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
        if reverse {
            packages.reverse();
            modules.reverse();
        }
        let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
        (
            CompileUnitInput::new(&self.sources, packages, modules, Vec::new()),
            prelude,
        )
    }
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
