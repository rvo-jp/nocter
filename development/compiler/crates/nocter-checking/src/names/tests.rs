use nocter_compile_input::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageInput, PackageMode, UseResolutionInput,
};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_model::PackageIdentity;
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::{NodeId, NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

use super::{
    CaptureMode, LocalBindingKind, NameTarget, resolve_body_names,
    resolve_cataloged_body_names_recovering,
};

#[test]
fn lexical_identities_cover_scopes_and_explicit_capture_projection() {
    let fixture = Fixture::new(
        "func main(input: i32, arena: i32): void {\n    let first = input\n    for item in input ..< input {\n        let inside = item\n    }\n    region temp using arena {\n        let nested = temp\n    }\n    if input is State.some(payload) {\n        let branch = payload\n    }\n    let closure = (&first; value: i32): i32 { value + first }\n    drop first\n    return\n}\n",
        "",
    );
    let input = fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let resolution =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap();
    let (_, body) = resolution
        .bodies()
        .iter()
        .find(|(_, body)| body.locals().len() > 1)
        .unwrap();

    assert_eq!(body.locals().len(), 9);
    assert_eq!(body.captures().len(), 1);
    assert_eq!(
        body.captures().iter().next().unwrap().1.mode(),
        CaptureMode::Readonly
    );
    assert_eq!(
        body.locals().iter().next().unwrap().1.kind(),
        LocalBindingKind::Immutable
    );
    assert!(
        body.locals()
            .iter()
            .any(|(_, local)| local.kind() == LocalBindingKind::PatternPayload)
    );
    let (capture, capture_data) = body.captures().iter().next().unwrap();
    assert!(matches!(capture_data.source(), NameTarget::Local(_)));
    assert!(
        body.uses()
            .iter()
            .any(|usage| usage.target() == NameTarget::Capture(capture))
    );
    assert!(body.scopes().iter().any(|(_, scope)| {
        scope
            .bindings()
            .iter()
            .any(|binding| matches!(binding.target(), NameTarget::Parameter(_)))
    }));
    assert!(body.scopes().iter().any(|(_, scope)| {
        scope
            .bindings()
            .iter()
            .any(|binding| matches!(binding.target(), NameTarget::Local(_)))
    }));
    assert!(body.scopes().iter().any(|(_, scope)| {
        scope
            .bindings()
            .iter()
            .any(|binding| binding.target() == NameTarget::Capture(capture))
    }));
    for (scope, _) in body.scopes().iter() {
        let projections = resolution
            .source_index()
            .bindings_for(SemanticEntity::BodyScope(body.body(), scope));
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].role(), SourceRole::Implementation);
        assert!(projections[0].origin().node().is_some());
    }

    let capture_bindings = resolution
        .source_index()
        .bindings_for(SemanticEntity::Capture(body.body(), capture));
    assert!(
        capture_bindings
            .iter()
            .any(|binding| binding.role() == SourceRole::Declaration)
    );
    assert!(
        capture_bindings
            .iter()
            .any(|binding| binding.role() == SourceRole::Reference)
    );
}

#[test]
fn binding_initializer_cannot_see_the_binding_being_declared() {
    let fixture = Fixture::new(
        "func main(): void {\n    let value = value\n    return\n}\n",
        "",
    );
    let input = fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let error =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0340");
}

#[test]
fn name_rule_retains_only_scopes_and_bindings_resolved_before_it() {
    let fixture = Fixture::new(
        concat!(
            "func main(input: i32): void {\n",
            "    let before = input\n",
            "    unknown\n",
            "    let after = input\n",
            "    return\n",
            "}\n",
            "func later(input: i32): void {\n",
            "    let retained = input\n",
            "    return\n",
            "}\n",
        ),
        "",
    );
    let input = fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let catalog = crate::catalog_body_sources(&input, program.graph(), &frontend_bindings).unwrap();
    let failure = resolve_cataloged_body_names_recovering(
        &input,
        program.graph(),
        &frontend_bindings,
        source_index,
        catalog,
    )
    .unwrap_err();

    assert_eq!(failure.error.source_diagnostic().unwrap().code(), "E0340");
    let recovery = failure.recovery.unwrap();
    let (_, body) = recovery.bodies.iter().next().unwrap();
    assert_eq!(
        body.rejection()
            .expect("rejected name evidence")
            .diagnostic()
            .code(),
        "E0340"
    );
    let body = body.usable_names().expect("failing body recovery");
    let names = body
        .scopes()
        .iter()
        .flat_map(|(_, scope)| scope.bindings())
        .filter_map(|binding| program.graph().symbols().spelling(binding.name()))
        .collect::<Vec<_>>();
    assert!(names.contains(&"input"));
    assert!(names.contains(&"before"));
    assert!(!names.contains(&"after"));
    assert!(body.scopes().iter().all(|(scope, _)| {
        !recovery
            .source_index
            .bindings_for(SemanticEntity::BodyScope(body.body(), scope))
            .is_empty()
    }));
    assert_eq!(recovery.bodies.iter().count(), 2);
    let later = recovery
        .bodies
        .iter()
        .filter_map(|(_, body)| body.usable_names())
        .find(|body| {
            body.locals().iter().any(|(_, local)| {
                program.graph().symbols().spelling(local.name()) == Some("retained")
            })
        });
    assert!(
        later.is_some(),
        "a prior name failure must not hide later bodies"
    );
}

#[test]
fn closure_outer_binding_requires_an_explicit_capture() {
    let fixture = Fixture::new(
        "func main(input: i32): void {\n    let closure = (value: i32): i32 { input + value }\n    return\n}\n",
        "",
    );
    let input = fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let error =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap_err();

    let diagnostic = error.source_diagnostic().unwrap();
    assert_eq!(diagnostic.code(), "E0346");
    assert_eq!(diagnostic.notes().len(), 1);
}

#[test]
fn closure_capture_rules_distinguish_missing_and_duplicate_targets() {
    for (source, expected) in [
        (
            "func main(): void {\n    let closure = (&missing;): i32 { 1 }\n    return\n}\n",
            "E0344",
        ),
        (
            "func main(input: i32): void {\n    let closure = (&input, move input;): i32 { 1 }\n    return\n}\n",
            "E0345",
        ),
    ] {
        let fixture = Fixture::new(source, "");
        let input = fixture.input(false, Vec::new());
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        let error = resolve_body_names(&input, program.graph(), &frontend_bindings, source_index)
            .unwrap_err();
        assert_eq!(error.source_diagnostic().unwrap().code(), expected);
    }
}

#[test]
fn authored_module_names_collide_but_prelude_fallback_is_shadowable() {
    let fixture = Fixture::new(
        "func helper(): void { return }\nfunc bad(helper: i32): void { return }\n",
        "pub struct print {}\n",
    );
    let input = fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let error =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap_err();
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0341");

    let shadow_fixture = Fixture::new(
        "func main(print: i32): void {\n    let value = print\n    return\n}\n",
        "pub struct print {}\n",
    );
    let input = shadow_fixture.input(false, Vec::new());
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap();
}

#[test]
fn block_import_resolves_to_export_without_creating_local_storage() {
    let fixture = Fixture::new(
        "func main(): void {\n    use lib.helper\n\n    helper()\n    return\n}\n",
        "",
    );
    let use_node = find_nodes(&fixture.app, NodeKind::BlockUseDeclaration)[0];
    let target = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["lib"]);
    let resolutions = vec![UseResolutionInput::new(use_node, target)];
    let input = fixture.input_with_library(false, resolutions);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let resolution =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap();
    let (_, body) = resolution
        .bodies()
        .iter()
        .find(|(_, body)| {
            body.uses()
                .iter()
                .any(|usage| matches!(usage.target(), NameTarget::Exported(_)))
        })
        .unwrap();

    assert_eq!(body.locals().len(), 0);
    assert!(
        body.uses()
            .iter()
            .any(|usage| matches!(usage.target(), NameTarget::Exported(_)))
    );
    assert!(body.scopes().iter().any(|(_, scope)| {
        scope
            .bindings()
            .iter()
            .any(|binding| matches!(binding.target(), NameTarget::Exported(_)))
    }));
}

#[test]
fn missing_selected_block_import_has_its_own_rule() {
    let fixture = Fixture::new(
        "func main(): void {\n    use lib.missing\n\n    return\n}\n",
        "",
    );
    let use_node = find_nodes(&fixture.app, NodeKind::BlockUseDeclaration)[0];
    let target = ModuleIdentity::new(PackageIdentity::new("workspace:app"), ["lib"]);
    let resolutions = vec![UseResolutionInput::new(use_node, target)];
    let input = fixture.input_with_library(false, resolutions);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let error =
        resolve_body_names(&input, program.graph(), &frontend_bindings, source_index).unwrap_err();

    assert_eq!(error.source_diagnostic().unwrap().code(), "E0342");
}

#[test]
fn body_name_diagnostic_is_independent_of_package_and_module_input_order() {
    let fixture = Fixture::new(
        "func main(input: i32): void {\n    let closure = (value: i32): i32 { input + value }\n    return\n}\n",
        "",
    );
    let mut diagnostics = Vec::new();
    for reverse in [false, true] {
        let input = fixture.input(reverse, Vec::new());
        let lowered = lower_compile_unit_declarations(&input).unwrap();
        let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
        diagnostics.push(
            resolve_body_names(&input, program.graph(), &frontend_bindings, source_index)
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
    library: SyntaxTree,
}

impl Fixture {
    fn new(app: &str, prelude: &str) -> Self {
        let mut sources = SourceMap::new();
        let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
        let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
        let app_id = add_source(&mut sources, "/app/index.nct", app);
        let std_id = add_source(
            &mut sources,
            "/std/index.nct",
            crate::test_support::BUILTIN_DECLARATIONS,
        );
        let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", prelude);
        let library_id = add_source(&mut sources, "/app/lib/index.nct", "pub struct helper {}\n");
        Self {
            app_manifest: parse_source(&sources, app_manifest_id, ParseGoal::SourceFile),
            std_manifest: parse_source(&sources, std_manifest_id, ParseGoal::SourceFile),
            app: parse_source(&sources, app_id, ParseGoal::SourceFile),
            standard: parse_source(&sources, std_id, ParseGoal::SourceFile),
            prelude: parse_source(&sources, prelude_id, ParseGoal::SourceFile),
            library: parse_source(&sources, library_id, ParseGoal::SourceFile),
            sources,
        }
    }

    fn input(&self, reverse: bool, resolutions: Vec<UseResolutionInput>) -> CompileUnitInput<'_> {
        self.build_input(reverse, resolutions, false)
    }

    fn input_with_library(
        &self,
        reverse: bool,
        resolutions: Vec<UseResolutionInput>,
    ) -> CompileUnitInput<'_> {
        self.build_input(reverse, resolutions, true)
    }

    fn build_input(
        &self,
        reverse: bool,
        resolutions: Vec<UseResolutionInput>,
        include_library: bool,
    ) -> CompileUnitInput<'_> {
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
        if include_library {
            modules.push(module(
                "workspace:app",
                &["lib"],
                "/app/lib/index.nct",
                &self.library,
            ));
        }
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
            resolutions,
        )
        .with_toolchain(crate::test_support::builtin_toolchain(
            &self.sources,
            &self.standard,
            prelude,
        ))
    }
}

fn find_nodes(tree: &SyntaxTree, kind: NodeKind) -> Vec<NodeId> {
    let mut found = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            found.push(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    found
}

fn add_source(sources: &mut SourceMap, name: &str, text: &str) -> SourceId {
    sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap()
}

fn parse_source(sources: &SourceMap, source: SourceId, goal: ParseGoal) -> SyntaxTree {
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
