//! The selected v2 package grammar converges on the existing physical/semantic authorities.
use super::*;
use nocter_checking::value_access::accepted::{ModuleSourceUnit, SourceFailure};
use nocter_declaration_lowering::access_declarations::{
    Failure, ModuleSourcesFailure, SourceRoleRule,
};
use nocter_syntax::ParseGoal;

fn request(
    tree: &TempTree,
    provider: &mut dyn SourceSyntaxProvider,
) -> (DiscoveryRequest, nocter_syntax::NodeId) {
    tree.source(
        "std/index.nct",
        "#package: { name: \"std\", version: \"2.0.0\" }",
    );
    let app = fs::canonicalize(tree.path().join("app")).unwrap();
    let std = fs::canonicalize(tree.path().join("std")).unwrap();
    let packages = ResolvedPackageGraph::load_with_root_catalog(
        vec![
            package("workspace:app", "app", &app),
            package("workspace:std", "std", &std),
        ],
        PackageRootCatalog::for_goal(SourceOverlay::empty(), ParseGoal::V2CoreUnit),
        nocter_source::SourceIdentityDomain::new(),
        provider,
    )
    .unwrap();
    let root_id = packages
        .syntax_trees()
        .iter()
        .find(|syntax| {
            packages
                .sources()
                .get(syntax.source())
                .unwrap()
                .name()
                .as_str()
                == app.join("index.nct").to_str().unwrap()
        })
        .unwrap()
        .root_id();
    let request = DiscoveryRequest::declared(
        CompilationTarget::Arm64Darwin,
        packages,
        vec![module("workspace:app", &[])],
        minimal_toolchain("workspace:std"),
    );
    (request, root_id)
}

#[test]
fn v2_directory_packages_retain_root_syntax_through_discovery_and_required_checking() {
    let tree = TempTree::new();
    tree.source("app/index.nct", "#package: { name: \"app\", version: \"2.0.0\" }\n#executable: { name: \"app\" }\nsee ./body.nct\nuse ./codec\nconst func run(take value: i32): i32\nconst RESULT: i32");
    tree.source("app/body.nct", "see ./index.nct\nuse ./codec\nconst func run(take value: i32): i32 { value + codec.answer() }\nconst RESULT: i32 = run(2)");
    tree.source(
        "app/codec/index.nct",
        "see ./body.nct\nconst func answer(): i32",
    );
    tree.source(
        "app/codec/body.nct",
        "see ./index.nct\nconst func answer(): i32 { 40 }",
    );
    let mut provider = CountingSourceSyntax::default();
    let (request, root_id) = request(&tree, &mut provider);
    let discovered = discover_with_source_syntax(request, &mut provider).unwrap();
    assert_eq!(discovered.source_goal(), ParseGoal::V2CoreUnit);
    assert_eq!(discovered.syntax_trees().len(), 5);
    assert!(
        discovered
            .syntax_trees()
            .iter()
            .all(|syntax| syntax.root().kind() == NodeKind::V2CoreUnit)
    );
    assert!(
        discovered
            .syntax_trees()
            .iter()
            .any(|syntax| syntax.root_id() == root_id)
    );
    assert_eq!(
        discovered
            .compile_input()
            .unwrap()
            .package_target_resolutions()
            .len(),
        1
    );
    let checked = ModuleSourceUnit::prepare(discovered.compile_input().unwrap()).unwrap();
    let current = checked.declarations().current().unwrap();
    let program = current.program();
    let result = program
        .resolve_constant_in(
            current.lookup_scope(),
            program.symbols().get("RESULT").unwrap(),
        )
        .unwrap();
    assert_eq!(
        checked.unit().constant_value(result).unwrap().node(),
        &nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(42))
    );
    checked.unit().bind(&current).unwrap();
}

#[test]
fn package_metadata_in_ordinary_sources_or_child_roots_retains_the_real_diagnostic() {
    use nocter_declaration_lowering::{LoweringError, SurfaceError, TopologyRule};
    for (suffix, wrong_path) in [("", "app/body.nct"), ("use ./codec", "app/codec/index.nct")] {
        let tree = TempTree::new();
        tree.source(
            "app/index.nct",
            &format!("#package: {{ name: \"app\", version: \"2.0.0\" }}\n{suffix}"),
        );
        tree.source(
            wrong_path,
            "#dependencies: { local: { path: \"./local\" } }",
        );
        let mut provider = CountingSourceSyntax::default();
        let (request, _) = request(&tree, &mut provider);
        let unit = discover_with_source_syntax(request, &mut provider).unwrap();
        let wrong = fs::canonicalize(tree.path().join(wrong_path)).unwrap();
        let source = unit
            .sources()
            .find_by_name(wrong.to_str().unwrap())
            .unwrap()
            .id();
        let Err(failure) = ModuleSourceUnit::prepare(unit.compile_input().unwrap()) else {
            panic!("package metadata outside the package root must reject")
        };
        let report = failure.project(unit.compile_input().unwrap()).unwrap();
        let SourceFailure::Modules(ModuleSourcesFailure::Surface(SurfaceError::Topology(
            LoweringError::Rule(error),
        ))) = report.failure()
        else {
            panic!("original topology cause")
        };
        assert_eq!(report.diagnostics().len(), 1);
        assert_eq!(report.diagnostics()[0].code(), "E0276");
        assert_eq!(report.diagnostics()[0].primary().source(), source);
        assert_eq!(error.rule(), TopologyRule::PackageDirectiveOutsideRoot);
        let nocter_syntax::SyntaxOrigin::Node(subject) = error.primary() else {
            panic!("package directive node")
        };
        assert_eq!(subject.source(), source);
    }
}

#[test]
fn package_metadata_does_not_authorize_executable_contract_roots() {
    for (item, expected) in [
        ("func run(): i32 { 42 }", SourceRoleRule::BodyInContract),
        (
            "const RESULT: i32 = 42",
            SourceRoleRule::InitializerInContract,
        ),
    ] {
        let tree = TempTree::new();
        tree.source(
            "app/index.nct",
            &format!("#package: {{ name: \"app\", version: \"2.0.0\" }}\n{item}"),
        );
        let mut provider = CountingSourceSyntax::default();
        let (request, root) = request(&tree, &mut provider);
        let unit = discover_with_source_syntax(request, &mut provider).unwrap();
        let Err(SourceFailure::Declarations(Failure::SourceRole(error))) =
            ModuleSourceUnit::prepare(unit.compile_input().unwrap())
        else {
            panic!("an implementation does not belong in a contract root")
        };
        assert_eq!(error.rule(), expected);
        assert_eq!(error.source().primary().source(), root.source());
    }
}

#[test]
fn v2_see_cannot_cross_module_or_nested_package_boundaries() {
    for nested_package in [false, true] {
        let tree = TempTree::new();
        tree.source(
            "app/index.nct",
            "#package: { name: \"app\", version: \"2.0.0\" }\nsee ./nested/body.nct",
        );
        tree.source(
            "app/nested/index.nct",
            if nested_package {
                "#package: { name: \"nested\", version: \"2.0.0\" }"
            } else {
                ""
            },
        );
        tree.source("app/nested/body.nct", "func hidden(): i32 { 42 }");
        let mut provider = CountingSourceSyntax::default();
        let (request, _) = request(&tree, &mut provider);
        let Err(failure) = discover_with_source_syntax(request, &mut provider) else {
            panic!("cross-boundary see")
        };
        assert!(
            matches!(failure.error(), DiscoveryError::SourceVisibility { failure, .. } if matches!(
                (nested_package, failure),
                (false, crate::SourceVisibilityFailure::CrossesModule { .. }) |
                (true, crate::SourceVisibilityFailure::CrossesPackage { .. })
            ))
        );
    }
}

#[test]
fn v2_inventory_checks_unused_source_folder_helpers() {
    use nocter_checking::value_access::{Failure as BodyFailure, Rule, accepted};
    let tree = TempTree::new();
    tree.source(
        "app/index.nct",
        "#package: { name: \"app\", version: \"2.0.0\" }",
    );
    tree.source("app/details/unused.nct", "func unused(): i32 { missing }");
    let mut provider = CountingSourceSyntax::default();
    let (request, _) = request(&tree, &mut provider);
    let unit = discover_with_source_syntax(request, &mut provider).unwrap();
    let path = fs::canonicalize(tree.path().join("app/details/unused.nct")).unwrap();
    let source = unit
        .sources()
        .find_by_name(path.to_str().unwrap())
        .unwrap()
        .id();
    let Err(SourceFailure::Checking(accepted::Failure::Body {
        failure: BodyFailure::Rejected(error),
        ..
    })) = ModuleSourceUnit::prepare(unit.compile_input().unwrap())
    else {
        panic!("unused helper must be checked")
    };
    assert_eq!(error.rule(), Rule::UnknownName);
    assert_eq!(error.source().primary().source(), source);
}
