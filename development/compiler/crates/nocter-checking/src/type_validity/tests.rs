use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageInput, PackageMode, ToolchainInput,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::PackageIdentity;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::validate_declaration_types;

#[test]
fn valid_special_roots_and_indirections_are_accepted() {
    let fixture = Fixture::new(
        "type Completion = void\ntype Divergence = never\ntype Bytes = [u8]\nfunc stop(): never {}\nfunc perform(): void! {}\nfunc inspect(bytes: &[u8], pointer: *void): void {}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, types) = program.into_parts();

    validate_declaration_types(&graph, &types, &source_index).unwrap();
}

#[test]
fn invalid_type_positions_have_distinct_rules() {
    for (source, expected) in [
        ("struct Bad { value: (i32?)? }\n", "E0360"),
        ("struct Bad { value: void? }\n", "E0361"),
        ("struct Bad { value: never! }\n", "E0362"),
        ("func bad(value: never): void {}\n", "E0363"),
        ("struct Bad { value: void }\n", "E0364"),
        ("func bad(value: str): void {}\n", "E0365"),
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let (graph, types) = program.into_parts();
        let error = validate_declaration_types(&graph, &types, &source_index).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), expected);
    }
}

#[test]
fn aliases_do_not_bypass_use_site_validity() {
    let fixture = Fixture::new("type Completion = void\nstruct Bad { value: Completion }\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, types) = program.into_parts();
    let error = validate_declaration_types(&graph, &types, &source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0364");
}

#[test]
fn associated_bindings_and_refinements_are_data_positions() {
    for source in [
        "pub interface Source { pub type Item }\nstruct Value {}\nconform Source for Value { type Item = void }\n",
        "struct Box<T> {}\ninstance Box<T> where T = void {}\n",
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let (graph, types) = program.into_parts();
        let error = validate_declaration_types(&graph, &types, &source_index).unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), "E0364");
    }
}

#[test]
fn type_validity_diagnostic_is_input_order_independent() {
    let fixture = Fixture::new("struct Bad { value: void }\n");
    let mut diagnostics = Vec::new();
    for reverse in [false, true] {
        let input = fixture.input(reverse);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let (graph, types) = program.into_parts();
        diagnostics.push(
            validate_declaration_types(&graph, &types, &source_index)
                .unwrap_err()
                .source_diagnostic()
                .unwrap()
                .clone(),
        );
    }
    assert_eq!(diagnostics[0], diagnostics[1]);
}

struct Fixture {
    sources: SourceMap,
    app_manifest: SyntaxTree,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    standard: SyntaxTree,
    prelude: SyntaxTree,
}

impl Fixture {
    fn new(app: &str) -> Self {
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

    fn input(&self, reverse: bool) -> CompileUnitInput<'_> {
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
        CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            Vec::new(),
        )
        .with_toolchain(ToolchainInput::new(
            PackageIdentity::new("toolchain:std"),
            prelude,
            Vec::new(),
            Vec::new(),
        ))
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
