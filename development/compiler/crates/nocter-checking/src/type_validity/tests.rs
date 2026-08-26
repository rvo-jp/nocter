use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::PackageIdentity;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::validate_declaration_types;
use crate::prepare_program_checking;

#[test]
fn valid_special_roots_and_indirections_are_accepted() {
    let fixture = Fixture::new(
        "type Completion = void\ntype Divergence = never\ntype Bytes = [u8]\nfunc stop(): never {}\nfunc perform(): void! {}\nfunc inspect(bytes: &[u8], pointer: *void): void {}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, types, _admission) = program.into_parts();

    validate_declaration_types(&graph, &types, source_index.diagnostic_origins()).unwrap();
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
        ("struct Bad { value: func(...str): void }\n", "E0365"),
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let (graph, types, _admission) = program.into_parts();
        let error = validate_declaration_types(&graph, &types, source_index.diagnostic_origins())
            .unwrap_err();

        assert_eq!(error.source_diagnostic().unwrap().code(), expected);
    }
}

#[test]
fn aliases_do_not_bypass_use_site_validity() {
    let fixture = Fixture::new("type Completion = void\nstruct Bad { value: Completion }\n");
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
    let (graph, types, _admission) = program.into_parts();
    let error =
        validate_declaration_types(&graph, &types, source_index.diagnostic_origins()).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0364");
}

#[test]
fn associated_bindings_and_refinements_are_data_positions() {
    for source in [
        "pub interface Source { pub type Item }\nstruct Value {}\ninstance Value { impl Source { .Item = void } }\n",
        "struct Box<T> {}\ninstance Box<T> where T = void {}\n",
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts(&input);
        let (graph, types, _admission) = program.into_parts();
        let error = validate_declaration_types(&graph, &types, source_index.diagnostic_origins())
            .unwrap_err();

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
        let (graph, types, _admission) = program.into_parts();
        diagnostics.push(
            validate_declaration_types(&graph, &types, source_index.diagnostic_origins())
                .unwrap_err()
                .source_diagnostic()
                .unwrap()
                .clone(),
        );
    }
    assert_eq!(diagnostics[0], diagnostics[1]);
}

#[test]
fn concrete_associated_projection_requires_an_applicable_implementation() {
    let source = concat!(
        "interface Source { pub type Item }\n",
        "struct Wrapper<T> {}\n",
        "instance Wrapper<T> where T = i32 { impl Source { .Item = i32 } }\n",
        "func invalid(value: Wrapper<i64>): Wrapper<i64>.Item { return value }\n",
    );
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, bindings, source_index) = lowered.into_checking_parts(&input);
    let error = prepare_program_checking(&input, program, &bindings, source_index).unwrap_err();
    let diagnostic = error.source_diagnostic().unwrap();

    assert_eq!(diagnostic.code(), "E0367");
    assert_eq!(
        diagnostic.primary().span().range().start().get(),
        u32::try_from(source.find("Item { return").unwrap()).unwrap()
    );
}

#[test]
fn concrete_associated_projection_rejects_multiple_interface_applications() {
    let source = concat!(
        "interface Source<T> { pub type Item }\n",
        "struct Buffer {}\n",
        "instance Buffer { impl Source<i32> { .Item = i32 } }\n",
        "instance Buffer { impl Source<i64> { .Item = i64 } }\n",
        "func invalid(value: Buffer): Buffer.Item { return value }\n",
    );
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, bindings, source_index) = lowered.into_checking_parts(&input);
    let error = prepare_program_checking(&input, program, &bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0368");
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
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", app);
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::BUILTIN_DECLARATIONS,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
        Self {
            app_manifest: parsed(&sources, app_manifest_id, ParseGoal::SourceFile),
            std_manifest: parsed(&sources, std_manifest_id, ParseGoal::SourceFile),
            app: parsed(&sources, app_id, ParseGoal::SourceFile),
            standard: parsed(&sources, std_id, ParseGoal::SourceFile),
            prelude: parsed(&sources, prelude_id, ParseGoal::SourceFile),
            sources,
        }
    }

    fn input(&self, reverse: bool) -> CompileUnitInput<'_> {
        let mut packages = vec![
            package("workspace:app", "app", "/app/index.nct", &self.app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &self.std_manifest),
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
        .with_toolchain(crate::test_support::builtin_toolchain(
            &self.sources,
            &self.standard,
            prelude,
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

fn package(identity: &str, name: &str, _path: &str, _manifest: &SyntaxTree) -> PackageInput {
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
