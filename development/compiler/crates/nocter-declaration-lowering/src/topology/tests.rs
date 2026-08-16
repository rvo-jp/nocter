use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{LoweringError, lower_compile_unit_topology};
use crate::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
};

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
    parse(sources.get(source).unwrap(), goal)
}

fn declared_package<'syntax>(
    identity: &str,
    name: &str,
    path: &str,
    declaration: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        PackageIdentity::new(identity),
        name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, declaration)),
    )
}

fn root_module<'syntax>(
    package: &str,
    sources: Vec<ModuleSourceInput<'syntax>>,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new(package), Vec::<&str>::new()),
        sources,
    )
}

#[test]
fn input_order_does_not_change_semantic_topology() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let app_root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub func run(): void { return }\n",
    );
    let app_impl_id = add_source(
        &mut sources,
        "/app/support.nct",
        "func support(value: usize): usize { value }\n",
    );
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "#name: \"dep\"\n");
    let dep_root_id = add_source(
        &mut sources,
        "/dep/index.nct",
        "pub struct Item { value: usize }\n",
    );

    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let app_root = parse_source(&sources, app_root_id, ParseGoal::ModuleSource);
    let app_impl = parse_source(&sources, app_impl_id, ParseGoal::ModuleSource);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let dep_root = parse_source(&sources, dep_root_id, ParseGoal::ModuleSource);

    let packages = vec![
        declared_package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
        declared_package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
    ];
    let modules = vec![
        root_module(
            "resolved:dep",
            vec![ModuleSourceInput::new(
                "/dep/index.nct",
                ModuleSourceKind::Root,
                &dep_root,
            )],
        ),
        root_module(
            "workspace:app",
            vec![
                ModuleSourceInput::new(
                    "/app/support.nct",
                    ModuleSourceKind::Implementation,
                    &app_impl,
                ),
                ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &app_root),
            ],
        ),
    ];

    let forward = lower_compile_unit_topology(&CompileUnitInput::new(
        &sources,
        packages.clone(),
        modules.clone(),
    ))
    .unwrap();
    let reverse = lower_compile_unit_topology(&CompileUnitInput::new(
        &sources,
        packages.into_iter().rev().collect(),
        modules
            .into_iter()
            .rev()
            .map(|module| {
                let mut source_order = module.sources().to_vec();
                source_order.reverse();
                ModuleInput::new(module.identity().clone(), source_order)
            })
            .collect(),
    ))
    .unwrap();

    assert_eq!(forward.program().symbols(), reverse.program().symbols());
    assert_eq!(forward.program().packages(), reverse.program().packages());
    assert_eq!(forward.program().modules(), reverse.program().modules());
    assert_eq!(forward.source_index(), reverse.source_index());
}

#[test]
fn rejects_a_physical_source_claimed_by_manifest_and_module() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let root_id = add_source(&mut sources, "/app/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/same.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![ModuleSourceInput::new(
            "/same.nct",
            ModuleSourceKind::Root,
            &root,
        )],
    );

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        &sources,
        vec![package],
        vec![module],
    ))
    .unwrap_err();

    assert_eq!(
        error,
        LoweringError::DuplicateSourcePath("/same.nct".into())
    );
}

#[test]
fn single_file_package_has_one_root_module_without_a_manifest() {
    let mut sources = SourceMap::new();
    let source_id = add_source(
        &mut sources,
        "/tmp/example.nct",
        "func main(): void { return }\n",
    );
    let syntax = parse_source(&sources, source_id, ParseGoal::ModuleSource);
    let package = PackageInput::new(
        PackageIdentity::new("single:/tmp/example.nct"),
        "example",
        PackageMode::SingleFile,
        None,
    );
    let module = root_module(
        "single:/tmp/example.nct",
        vec![ModuleSourceInput::new(
            "/tmp/example.nct",
            ModuleSourceKind::SingleFile,
            &syntax,
        )],
    );

    let lowered = lower_compile_unit_topology(&CompileUnitInput::new(
        &sources,
        vec![package],
        vec![module],
    ))
    .unwrap();

    assert_eq!(lowered.program().packages().len(), 1);
    assert_eq!(lowered.program().modules().len(), 1);
    assert_eq!(lowered.source_index().len(), 1);
}

#[test]
fn package_mode_cannot_be_smuggled_through_another_source_layout() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "#name: \"app\"\n");
    let source_id = add_source(&mut sources, "/app/index.nct", "");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let syntax = parse_source(&sources, source_id, ParseGoal::ModuleSource);
    let package = declared_package("workspace:app", "app", "/app/nocter.nct", &manifest);
    let module = root_module(
        "workspace:app",
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::SingleFile,
            &syntax,
        )],
    );

    let error = lower_compile_unit_topology(&CompileUnitInput::new(
        &sources,
        vec![package],
        vec![module],
    ))
    .unwrap_err();

    assert!(matches!(error, LoweringError::InvalidModuleLayout(_)));
}
