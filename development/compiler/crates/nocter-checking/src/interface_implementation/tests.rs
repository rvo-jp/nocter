use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::ProvenanceOrigin;
use nocter_model::PackageIdentity;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{MethodSelection, build_interface_implementation_table};

#[test]
fn required_and_default_methods_receive_exact_dispatch_selections() {
    let fixture = Fixture::new(
        "pub interface Readable {\n    pub method &self.read(offset: i32): i32\n    pub default method &self.default_value(): i32 { return 1 }\n}\nstruct Value {}\ninstance Value {\n    impl Readable\n    method &self.read(offset: i32): i32 { return offset }\n}\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
    let (graph, mut types, _admission) = program.into_parts();
    let table =
        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap();
    let (_, entry) = table.entries().iter().next().unwrap();

    assert_eq!(entry.methods().len(), 2);
    assert!(
        entry
            .methods()
            .iter()
            .any(|method| { matches!(method.selection(), MethodSelection::Implementation(_)) })
    );
    assert!(
        entry
            .methods()
            .iter()
            .any(|method| { matches!(method.selection(), MethodSelection::Default(_)) })
    );
    assert_eq!(table.candidates(entry.interface().interface()).len(), 1);
    for method in entry.methods() {
        assert_eq!(
            method.selected_input(ProvenanceOrigin::Receiver),
            Some(ProvenanceOrigin::Receiver)
        );
        let interface = graph
            .declarations()
            .callables()
            .get(method.interface_method())
            .unwrap();
        let selected = match method.selection() {
            MethodSelection::Implementation(selected) | MethodSelection::Default(selected) => {
                graph.declarations().callables().get(selected).unwrap()
            }
        };
        for (interface, selected) in interface.parameters().iter().zip(selected.parameters()) {
            assert_eq!(
                method.selected_input(ProvenanceOrigin::Parameter(*interface)),
                Some(ProvenanceOrigin::Parameter(*selected))
            );
        }
    }
}

#[test]
fn interface_implementation_method_failures_have_distinct_rules() {
    for (source, expected) in [
        (
            "pub interface Readable { pub method &self.read(): i32 }\nstruct Value {}\ninstance Value { impl Readable }\n",
            "E0350",
        ),
        (
            "pub interface Readable { pub method &self.read(): i32 }\nstruct Value {}\ninstance Value {\n    impl Readable\n    method &self.read(): u32 { return 0 }\n}\n",
            "E0352",
        ),
        (
            "pub interface Readable { pub method &self.read(...values: i32): i32 }\nstruct Value {}\ninstance Value {\n    impl Readable\n    method &self.read(value: i32): i32 { return value }\n}\n",
            "E0352",
        ),
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
        let (graph, mut types, _admission) = program.into_parts();
        let error = build_interface_implementation_table(
            &graph,
            &mut types,
            source_index.diagnostic_origins(),
        )
        .unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), expected);
    }
}

#[test]
fn missing_method_failure_retains_every_specialized_required_signature() {
    let fixture = Fixture::new(concat!(
        "pub interface Readable {\n",
        "    pub type Item\n",
        "    pub method &self.read<T>(fallback: T): Self.Item from self where copy T\n",
        "    pub method &self.ready(): bool\n",
        "}\n",
        "struct Value {}\n",
        "instance Value {\n",
        "    impl Readable { .Item = i32 }\n",
        "}\n",
    ));
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
    let (graph, mut types, _admission) = program.into_parts();
    let error =
        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap_err();
    let missing = error.missing_methods().unwrap();

    assert_eq!(missing.required().len(), 2);
    let read = &missing.required()[0];
    assert_eq!(read.parameters().len(), 1);
    assert_eq!(read.generic_parameters().len(), 1);
    assert_eq!(read.requirements().len(), 1);
    assert_eq!(
        types.get(read.result()),
        Some(&nocter_model::TypeKind::Builtin(
            nocter_model::BuiltinType::I32
        ))
    );
}

#[test]
fn exact_overlap_diagnostic_is_input_order_independent() {
    let fixture = Fixture::new(
        "pub interface Marker {}\nstruct Value {}\ninstance Value { impl Marker }\ninstance Value { impl Marker }\n",
    );
    let mut diagnostics = Vec::new();
    for reverse in [false, true] {
        let input = fixture.input(reverse);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
        let (graph, mut types, _admission) = program.into_parts();
        diagnostics.push(
            build_interface_implementation_table(
                &graph,
                &mut types,
                source_index.diagnostic_origins(),
            )
            .unwrap_err()
            .source_diagnostic()
            .unwrap()
            .clone(),
        );
    }
    assert_eq!(diagnostics[0], diagnostics[1]);
    assert_eq!(diagnostics[0].code(), "E0353");
}

#[test]
fn refined_pattern_overlaps_a_general_generic_pattern() {
    let fixture = Fixture::new(
        "pub interface Marker<T> {}\nstruct Box<T> {}\ninstance Box<T> { impl Marker<T> }\ninstance Box<U> where U = i32 { impl Marker<U> }\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
    let (graph, mut types, _admission) = program.into_parts();
    let error =
        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0353");
}

#[test]
fn distinct_refinements_produce_disjoint_canonical_patterns() {
    let fixture = Fixture::new(
        "pub interface Marker<T> {}\nstruct Box<T> {}\ninstance Box<T> where T = i32 { impl Marker<T> }\ninstance Box<U> where U = u32 { impl Marker<U> }\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
    let (graph, mut types, _admission) = program.into_parts();
    let table =
        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap();

    assert_eq!(table.entries().len(), 2);
    for interface_implementation in table.entries().values() {
        assert!(interface_implementation.requirements().is_empty());
        assert_eq!(interface_implementation.generic_parameters().len(), 1);
        assert_eq!(interface_implementation.refinements().len(), 1);
        let nocter_model::TypeKind::Nominal { arguments, .. } =
            types.get(interface_implementation.target()).unwrap()
        else {
            panic!("refined interface_implementation target must remain nominal");
        };
        assert!(matches!(
            types.get(arguments[0]),
            Some(nocter_model::TypeKind::Builtin(
                nocter_model::BuiltinType::I32 | nocter_model::BuiltinType::U32
            ))
        ));
        assert_eq!(interface_implementation.refinements()[0].ty(), arguments[0]);
    }
}

#[test]
fn associated_type_bounds_use_the_same_interface_implementation_table() {
    for source in [
        "pub interface Marker {}\npub interface Source { pub type Item impl Marker }\nstruct Good {}\nstruct Wrapper<T> {}\ninstance Good { impl Marker }\ninstance Wrapper<T> where T = Good { impl Source { .Item = T } }\n",
        "pub interface Marker {}\npub interface Source { pub type Item impl Marker }\nstruct Wrapper<T> {}\ninstance Wrapper<T> where T impl Marker { impl Source { .Item = T } }\n",
    ] {
        let fixture = Fixture::new(source);
        let input = fixture.input(false);
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
        let (graph, mut types, _admission) = program.into_parts();

        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap();
    }
}

#[test]
fn unsatisfied_associated_type_bound_has_its_own_rule() {
    let fixture = Fixture::new(
        "pub interface Marker {}\npub interface Source { pub type Item impl Marker }\nstruct Missing {}\nstruct Wrapper<T> {}\ninstance Wrapper<T> where T = Missing { impl Source { .Item = T } }\n",
    );
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, _frontend_bindings, source_index) = lowered.into_checking_parts();
    let (graph, mut types, _admission) = program.into_parts();
    let error =
        build_interface_implementation_table(&graph, &mut types, source_index.diagnostic_origins())
            .unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0354");
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
