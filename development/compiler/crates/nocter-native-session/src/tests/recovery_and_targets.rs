use super::*;

#[test]
fn body_failure_retains_preparation_and_exact_typed_interruption() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func helper(): i32 { 1 }\nfunc main(input: i32): void {\n    input.missing()\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert!(!analysis.diagnostics().is_empty());
    let semantic = analysis.semantic_evidence().unwrap();
    assert!(!semantic.graph().declarations().callables().is_empty());
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| matches!(
                semantic.body_names(body),
                Some(nocter_session::SemanticBodyNamesView::Available(_))
            ))
    );
    assert!(!semantic.source_index().is_empty());
    let primary = analysis.diagnostics()[0].primary();
    let interruption = semantic
        .interruption_overlapping(primary.source(), primary.span().range())
        .unwrap();
    assert_eq!(
        interruption.origin().span(),
        analysis.diagnostics().first().unwrap().primary().span()
    );
    assert!(matches!(
        interruption.kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn name_failure_retains_lexical_state_without_claiming_body_preparation() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "func main(input: i32): void {\n    let before = input\n    unknown\n    let after = input\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert_eq!(analysis.diagnostics()[0].code(), "E0340");
    let semantic = analysis.semantic_evidence().unwrap();
    assert!(!semantic.graph().declarations().callables().is_empty());
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| semantic.body_names(body).is_some())
    );
    assert!(!semantic.source_index().is_empty());
}

#[test]
fn interface_implementation_failure_retains_declarations_without_claiming_later_semantics() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "pub interface Readable { pub method &self.read(): i32 }\n",
            "struct Value {}\n",
            "instance Value { impl Readable }\n",
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::CompilationFailed);
    assert_eq!(analysis.diagnostics()[0].code(), "E0350");
    let declarations = analysis.semantic_evidence().unwrap();
    assert!(
        !declarations
            .graph()
            .declarations()
            .interface_implementations()
            .is_empty()
    );
    assert!(!declarations.source_index().is_empty());
}

#[test]
fn incomplete_member_syntax_retains_typed_receiver_context() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        "struct Text { value: i32 }\ninstance Text { pub method &self.len(): usize { 0 } }\nfunc inspect(value: &Text): void {\n    value.\n    return\n}\n",
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    let semantic = analysis.semantic_evidence().expect("typed syntax recovery");
    let diagnostic = analysis.unit().syntax_diagnostics()[0].primary();
    let recovery = semantic
        .interruption_overlapping(diagnostic.source(), diagnostic.span().range())
        .expect("expected body evidence");
    assert!(matches!(
        recovery.kind(),
        nocter_checking::TypedBodyInterruptionKind::MemberSelection { .. }
    ));
}

#[test]
fn incomplete_declaration_syntax_cannot_enter_body_recovery() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source("main.nct", "func broken(: void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(analysis.semantic_evidence().is_none());
}

#[test]
fn incomplete_syntax_preserves_an_independent_declaration_failure() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "pub interface Readable { pub method &self.read(): i32 }\n",
            "struct Value {}\n",
            "instance Value { impl Readable }\n",
            "func inspect(value: &Value): void {\n",
            "    value.\n",
            "    return\n",
            "}\n",
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0350")
    );
    let semantic = analysis.semantic_evidence().expect("declaration analysis");
    let declarations = semantic;
    assert!(
        !declarations
            .graph()
            .declarations()
            .interface_implementations()
            .is_empty()
    );
}

#[test]
fn incomplete_syntax_preserves_an_earlier_name_failure() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "main.nct",
        concat!(
            "struct Text {}\n",
            "func inspect(value: &Text): void {\n",
            "    unknown\n",
            "    value.\n",
            "    return\n",
            "}\n",
        ),
    );
    let standard_package = PackageIdentity::new("toolchain:std");
    let unit = discover(DiscoveryRequest::single_file(
        CompilationTarget::Arm64Darwin,
        package_root.0.join("main.nct"),
        package_graph(vec![resolved_standard(&standard_root, &standard_package)]),
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    assert!(unit.has_syntax_errors());
    let analysis = analyze_for_test(unit);
    assert_eq!(analysis.status(), AnalyzedUnitStatus::SyntaxFailed);
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code() == "E0340")
    );
    let semantic = analysis.semantic_evidence().expect("name analysis");
    assert!(
        semantic
            .graph()
            .declarations()
            .bodies()
            .iter()
            .any(|(body, _)| semantic.body_names(body).is_some())
    );
}

#[test]
fn all_root_executables_share_one_target_compilation_and_keep_declaration_order() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "index.nct",
        concat!(
            "//! Multi executable package.\n",
            "#package: { name: \"multi\", version: \"0.0.0\", }\n",
            "#executable: { name: \"first\", module: \"./first\" }\n",
            "#executable: { name: \"second\", module: \"./second\" }\n",
        ),
    );
    package_root.source("first/index.nct", "func main(): void { return }\n");
    package_root.source("second/index.nct", "func main(): void { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:multi");
    let compile = |reverse_input: bool| {
        let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
            .with_standard_dependency(standard_package.clone());
        let mut packages = vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ];
        let mut roots = vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["first"]),
            ModuleIdentity::new(package.clone(), ["second"]),
        ];
        if reverse_input {
            packages.reverse();
            roots.reverse();
        }
        let unit = discover(DiscoveryRequest::declared(
            CompilationTarget::Arm64Darwin,
            package_graph(packages),
            roots,
            bundled_standard_toolchain(&standard_package),
        ))
        .unwrap();
        let target = compile_for_test(unit);
        compile_native_images(NativeImageSetCompileRequest::all(target)).unwrap()
    };

    let image_set = compile(false);
    let reversed_image_set = compile(true);
    assert_eq!(
        image_set
            .entries()
            .iter()
            .map(|entry| entry.identity().name())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.identity().package() == &package)
    );
    assert!(
        image_set
            .entries()
            .iter()
            .all(|entry| entry.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe]))
    );
    assert_eq!(
        image_set
            .entries()
            .iter()
            .map(|entry| (entry.identity(), entry.image().bytes()))
            .collect::<Vec<_>>(),
        reversed_image_set
            .entries()
            .iter()
            .map(|entry| (entry.identity(), entry.image().bytes()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn native_test_set_preserves_target_and_case_declaration_identity() {
    let standard_root = nocter_test_support::standard_library_root();
    let package_root = TempPackage::new();
    package_root.source(
        "index.nct",
        concat!(
            "//! Test package.\n",
            "#package: { name: \"tests\", version: \"0.0.0\", }\n",
            "#test: { name: \"unit\", module: \"./unit\" }\n",
            "#test: { name: \"integration\", module: \"./integration\" }\n",
        ),
    );
    package_root.source(
        "unit/index.nct",
        "test first { return }\ntest second { return }\n",
    );
    package_root.source("integration/index.nct", "test external { return }\n");
    let standard_package = PackageIdentity::new("toolchain:std");
    let package = PackageIdentity::new("workspace:tests");
    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
        vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["unit"]),
            ModuleIdentity::new(package.clone(), ["integration"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();

    let target = compile_for_test(unit);
    let compiled = compile_native_tests(NativeTestCompileRequest::all(target)).unwrap();
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| target.identity().name())
            .collect::<Vec<_>>(),
        ["unit", "integration"]
    );
    assert_eq!(
        compiled
            .targets()
            .iter()
            .map(|target| match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .map(|case| case.identity().name())
                    .collect::<Vec<_>>(),
                NativeTestTargetOutcome::CompileFailed(error) => {
                    panic!("test target failed native compilation: {error}")
                }
            })
            .collect::<Vec<_>>(),
        [vec!["first", "second"], vec!["external"]]
    );
    assert!(compiled.targets().iter().all(|target| {
        target.identity().package() == &package
            && match target.outcome() {
                NativeTestTargetOutcome::Compiled(cases) => cases
                    .iter()
                    .all(|case| case.image().bytes().starts_with(&[0xcf, 0xfa, 0xed, 0xfe])),
                NativeTestTargetOutcome::CompileFailed(_) => false,
            }
    }));

    let resolved = ResolvedPackageSpec::new(package.clone(), &package_root.0)
        .with_standard_dependency(standard_package.clone());
    let unit = discover(DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        package_graph(vec![
            resolved,
            resolved_standard(&standard_root, &standard_package),
        ]),
        vec![
            ModuleIdentity::new(package.clone(), Vec::<&str>::new()),
            ModuleIdentity::new(package.clone(), ["unit"]),
            ModuleIdentity::new(package.clone(), ["integration"]),
        ],
        bundled_standard_toolchain(&standard_package),
    ))
    .unwrap();
    let target = compile_for_test(unit);
    let selected =
        compile_native_tests(NativeTestCompileRequest::case(target, "unit", "second")).unwrap();
    let NativeTestTargetOutcome::Compiled(cases) = selected.targets()[0].outcome() else {
        panic!("selected case failed native compilation")
    };
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].identity().name(), "second");
}
