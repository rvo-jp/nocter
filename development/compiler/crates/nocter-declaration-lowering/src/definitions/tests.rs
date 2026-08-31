use nocter_declarations::{
    CallableKind, CallableProvenanceContract, ExportedEntity, NominalShape, ProvenanceAnnotation,
};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use crate::test_support::source_see;

const FULL_HEADER_SOURCE: &str = r#"
pub const BASE: usize = 40
const ANSWER: usize = BASE + 2
const STABLE: bool = false && (1 / 0 == 0)
const WIDE: u64 = 1
const EQUAL: bool = 1 == WIDE
const MINIMUM: i8 = -128
const LABEL: &str = "nocter"
type Bytes = [u8; ANSWER]

#target: "arm64-darwin"
pub struct Box<T> where copy T {
    pub value: T
}

enum Choice<T> {
    item(value: T)
    empty
}

type Alias<T> = Box<T> where copy T

pub interface Source<T> where copy T {
    pub type Item
    pub method &self.get(index: usize): &T from self
    pub method &self.static_view(): &str from static
}

construct Box<T> {
    pub func new(value: T): Self { return }
    pub literal [](...items: T): Self from items { return }
    pub literal ""(text: &str): Self from text { return }
}

instance Box<T> where copy T {
    pub method &self.view(): &T from self { return }
    pub coerce &self as &T from self { return }
    pub operator (&self == other: &Self): bool { return }
    pub operator (&self < other: &Self): bool { return }
    pub operator (&self[index: usize]): &T from self { return }
    pub operator (...&self): Box<T> from self { return }
}

instance Box<T> where copy T  {
    impl Source<T> { .Item = T }
    method &self.get(index: usize): &T from self { return }
    method &self.static_view(): &str from static { return }
}

func values<T>(value: &T): some Source<T> { .Item = &T } from value { return }
drop Box<T>(&+self) { return }
test headers { return }
"#;
use crate::{
    CompileUnitInput, DefinitionRule, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageIdentity, PackageInput, PackageMode, UseResolutionInput,
    apply_toolchain_profile, bind_header_type_syntax, collect_declaration_surface,
    evaluate_header_constants, normalize_header_types, prepare_authored_imports,
    prepare_declaration_headers, prepare_generic_binders, reserve_declaration_identities,
};

use super::define_declaration_headers_recovering;

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

fn lower<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    modules: Vec<ModuleInput<'syntax>>,
    source_visibilities: Vec<crate::SourceVisibilityResolutionInput>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> crate::LoweredDeclarations {
    try_lower(
        sources,
        packages,
        modules,
        source_visibilities,
        uses,
        prelude,
    )
    .unwrap()
}

fn try_lower<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput>,
    modules: Vec<ModuleInput<'syntax>>,
    source_visibilities: Vec<crate::SourceVisibilityResolutionInput>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> Result<crate::LoweredDeclarations, super::HeaderDefinitionError> {
    let builtin_source = modules
        .iter()
        .find(|module| {
            module.identity().package() == prelude.package() && module.identity().path().is_empty()
        })
        .and_then(|module| module.sources().first())
        .map(crate::ModuleSourceInput::syntax)
        .expect("test standard package has no root builtin source");
    let toolchain = crate::test_support::test_toolchain(
        prelude.clone(),
        &ModuleIdentity::new(prelude.package().clone(), Vec::<&str>::new()),
        builtin_source,
    );
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        sources,
        packages,
        modules,
        uses,
    )
    .with_source_visibility_resolutions(source_visibilities)
    .with_toolchain(toolchain.clone());
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface, &toolchain).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_toolchain_profile(imports).unwrap();
    let bound = bind_header_type_syntax(namespaces).unwrap();
    let bound = evaluate_header_constants(bound)?;
    let normalized = normalize_header_types(bound).unwrap();
    define_declaration_headers_recovering(normalized).map_err(|failure| failure.into_parts().0)
}

fn definition_error(source_text: &str) -> super::HeaderDefinitionError {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", source_text);
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let app_tree = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let toolchain = crate::test_support::test_toolchain(
        prelude.clone(),
        &ModuleIdentity::new(prelude.package().clone(), Vec::<&str>::new()),
        &std_root,
    );
    let input = CompileUnitInput::new(
        nocter_model::CompilationTarget::Arm64Darwin,
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app_tree),
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ],
        vec![],
    )
    .with_toolchain(toolchain.clone());
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface, &toolchain).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_toolchain_profile(imports).unwrap();
    let bound = bind_header_type_syntax(namespaces).unwrap();
    let bound = match evaluate_header_constants(bound) {
        Ok(bound) => bound,
        Err(error) => return error,
    };
    let normalized = normalize_header_types(bound).unwrap();
    define_declaration_headers_recovering(normalized)
        .unwrap_err()
        .into_parts()
        .0
}

#[test]
fn freezes_complete_header_graph_with_exact_leaf_ownership() {
    let (sources, lowered) = lower_full_header_program();
    let program = lowered.program();
    let declarations = program.declarations();

    assert_header_constants(program);

    assert_eq!(declarations.nominal_types().len(), 2);
    assert_eq!(declarations.fields().len(), 1);
    assert_eq!(declarations.variants().len(), 2);
    assert_eq!(declarations.interfaces().len(), 1);
    assert_eq!(declarations.associated_types().len(), 1);
    assert_eq!(declarations.constructions().len(), 1);
    assert_eq!(declarations.instances().len(), 1);
    assert_eq!(declarations.interface_implementations().len(), 1);
    assert_eq!(declarations.drops().len(), 1);
    assert_eq!(declarations.tests().len(), 1);
    assert_eq!(declarations.opaque_types().len(), 1);
    assert!(!declarations.parameters().is_empty());
    assert!(!declarations.requirements().is_empty());
    assert!(!declarations.bodies().is_empty());

    let app_package = program
        .packages()
        .iter()
        .find(|(_, package)| program.symbols().spelling(package.display_name()) == Some("app"))
        .map(|(id, _)| id)
        .unwrap();
    let app_module = program
        .modules()
        .iter()
        .find(|(_, module)| module.package() == app_package && module.path().segments().is_empty())
        .map(|(id, _)| id)
        .unwrap();
    let box_name = program.symbols().get("Box").unwrap();
    let shared_name = program.symbols().get("shared").unwrap();
    assert!(matches!(
        program.lookup_local(app_module, box_name),
        Some(ExportedEntity::NominalType(_))
    ));
    assert!(matches!(
        program.lookup_local(app_module, shared_name),
        Some(ExportedEntity::Callable(_))
    ));
    assert_eq!(
        program.lookup_export(app_module, app_module, shared_name),
        None,
        "prelude fallback must not become an authored export"
    );

    let (_, boxed) = declarations.nominal_types().iter().next().unwrap();
    assert!(matches!(
        boxed.shape(),
        NominalShape::Struct {
            copy_declared: false,
            fields
        } if fields.len() == 1
    ));
    assert!(declarations.callables().iter().any(|(_, callable)| {
        callable.kind() == CallableKind::Method
            && matches!(
                callable.provenance(),
                CallableProvenanceContract::Declared(_)
            )
    }));
    assert_provenance_annotations(declarations);
    assert!(lowered.source_index().len() > declarations.callables().len());
    assert_exact_unnamed_origins(&sources, &lowered);
}

fn lower_full_header_program() -> (SourceMap, crate::LoweredDeclarations) {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(
        &mut sources,
        "/std/prelude/index.nct",
        "pub func shared(): void { return }\n",
    );
    let app_id = add_source(&mut sources, "/app/index.nct", FULL_HEADER_SOURCE);
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let app_tree = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let lowered = lower(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app_tree),
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ],
        vec![],
        vec![],
        &prelude,
    );
    (sources, lowered)
}

fn assert_header_constants(program: &nocter_declarations::DeclarationProgram) {
    let values = program
        .declarations()
        .constants()
        .iter()
        .map(|(_, constant)| constant.value().clone())
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            nocter_model::ConstantValue::Integer(40),
            nocter_model::ConstantValue::Integer(42),
            nocter_model::ConstantValue::Bool(false),
            nocter_model::ConstantValue::Integer(1),
            nocter_model::ConstantValue::Bool(true),
            nocter_model::ConstantValue::Integer(-128),
            nocter_model::ConstantValue::Text("nocter".into()),
        ]
    );
    assert!(
        program
            .types()
            .iter()
            .any(|(_, ty)| matches!(ty, nocter_model::TypeKind::FixedArray { length: 42, .. }))
    );
}

fn assert_exact_unnamed_origins(sources: &SourceMap, lowered: &crate::LoweredDeclarations) {
    let declarations = lowered.program().declarations();
    let mut unnamed_origins = declarations
        .callables()
        .iter()
        .filter(|(_, callable)| callable.name().is_none())
        .map(|(id, _)| {
            let binding = lowered
                .source_index()
                .bindings_for(nocter_source_index::SemanticEntity::Callable(id))[0];
            sources
                .get(binding.origin().source())
                .unwrap()
                .text_at(binding.origin().span().range())
                .unwrap()
        })
        .collect::<Vec<_>>();
    unnamed_origins.sort_unstable();
    assert_eq!(unnamed_origins, ["\"\"", "...", "<", "==", "[", "[]", "as"]);

    let (opaque, _) = declarations.opaque_types().iter().next().unwrap();
    let opaque_origin = lowered
        .source_index()
        .bindings_for(nocter_source_index::SemanticEntity::OpaqueType(opaque))[0]
        .origin();
    assert_eq!(
        sources
            .get(opaque_origin.source())
            .unwrap()
            .text_at(opaque_origin.span().range()),
        Some("some")
    );
}

fn assert_provenance_annotations(declarations: &nocter_declarations::DeclarationArenas) {
    assert!(declarations.callables().iter().any(|(_, callable)| {
        matches!(
            callable.provenance_annotation(),
            ProvenanceAnnotation::Explicit {
                includes_static: true
            }
        )
    }));
    assert!(declarations.callables().iter().any(|(_, callable)| {
        callable.kind() == CallableKind::ConstructionFunction
            && callable.provenance_annotation() == ProvenanceAnnotation::Elided
    }));
}

#[test]
fn joins_contract_parameters_and_implementation_body_into_one_identity() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "see ./implementation.nct\n\npub func select<T>(value: &T): &T from value\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/implementation.nct",
        "see ./index.nct\n\nfunc select<T>(value: &T): &T from value { return }\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let app_module = ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/implementation.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
    );
    let lowered = lower(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            app_module,
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ],
        vec![
            source_see(&root, 0, "/app/implementation.nct"),
            source_see(&implementation, 0, "/app/index.nct"),
        ],
        vec![],
        &prelude,
    );
    let declarations = lowered.program().declarations();
    assert_eq!(declarations.callables().len(), 1);
    assert_eq!(declarations.parameters().len(), 1);
    assert_eq!(declarations.bodies().len(), 1);
    let (_, callable) = declarations.callables().iter().next().unwrap();
    let parameter = callable.parameters()[0];
    let body = callable.body().unwrap();
    let parameter_roles: Vec<_> = lowered
        .source_index()
        .bindings_for(nocter_source_index::SemanticEntity::Parameter(parameter))
        .iter()
        .map(|binding| binding.role())
        .collect();
    assert!(parameter_roles.contains(&nocter_source_index::SourceRole::Declaration));
    assert!(parameter_roles.contains(&nocter_source_index::SourceRole::Implementation));
    assert_eq!(
        lowered
            .source_index()
            .bindings_for(nocter_source_index::SemanticEntity::Body(body))[0]
            .role(),
        nocter_source_index::SourceRole::Implementation
    );
}

#[test]
fn rejects_ambiguous_bodyless_result_provenance() {
    for source in [
        "interface Choose {\n    pub method &self.choose(other: &Self): &Self\n}\n",
        "primitive func choose<T>(left: &T, right: &T): &T\n",
    ] {
        let error = definition_error(source);
        assert!(matches!(
            error,
            super::HeaderDefinitionError::Rule(violation)
                if violation.rule() == DefinitionRule::AmbiguousBodylessResultProvenance
        ));
    }
}

#[test]
fn rejects_argument_packs_outside_the_single_final_callable_position() {
    for source in [
        "func invalid(...items: i32, tail: i32): void { return }\n",
        "func invalid(...first: i32, ...second: i32): void { return }\n",
        "primitive func invalid(...items: i32): void\n",
        "enum Invalid { value(...items: i32) }\n",
    ] {
        assert!(matches!(
            definition_error(source),
            super::HeaderDefinitionError::Rule(violation)
                if violation.rule() == DefinitionRule::InvalidArgumentPackParameter
        ));
    }
}

#[test]
fn rejects_invalid_constant_contracts_without_runtime_fallback() {
    let cases = [
        (
            "const INVALID: str = \"value\"\n",
            DefinitionRule::InvalidConstantType,
        ),
        (
            "func make(): i32 { return 1 }\nconst INVALID: i32 = make()\n",
            DefinitionRule::NonConstantExpression,
        ),
        (
            "const INVALID: bool = 1\n",
            DefinitionRule::ConstantTypeMismatch,
        ),
        (
            "const FIRST: i32 = SECOND\nconst SECOND: i32 = FIRST\n",
            DefinitionRule::ConstantCycle,
        ),
        (
            "const INVALID: bool = false && 1\n",
            DefinitionRule::ConstantTypeMismatch,
        ),
        (
            "const INVALID: bool = false && INVALID\n",
            DefinitionRule::ConstantCycle,
        ),
        (
            "const INVALID: u8 = 255 + 1\n",
            DefinitionRule::ConstantArithmeticFailure,
        ),
    ];
    for (source, rule) in cases {
        let actual = definition_error(source);
        assert!(
            matches!(
                &actual,
                super::HeaderDefinitionError::Rule(violation) if violation.rule() == rule
            ),
            "{source}: expected {rule:?}, received {actual:?}"
        );
    }
}

#[test]
fn definition_rules_retain_exact_authored_subjects() {
    let cases = [
        (
            "func choose<T>(left: &T, right: &T): &T from missing { return }\n",
            DefinitionRule::UnknownResultProvenanceOrigin,
            "missing",
            None,
        ),
        (
            "func choose<T>(left: &T, right: &T): &T from left | left { return }\n",
            DefinitionRule::DuplicateResultProvenanceOrigin,
            "left {",
            Some("left |"),
        ),
        (
            "interface Source {\n    pub type Item\n}\nstruct Value {}\ninstance Value { impl Source { .Missing = i32 } }\n",
            DefinitionRule::UnknownAssociatedTypeBinding,
            "Missing",
            None,
        ),
        (
            "interface Source {\n    pub type Item\n}\nstruct Value {}\ninstance Value { impl Source { .Item = i32, .Item = i64 } }\n",
            DefinitionRule::DuplicateAssociatedTypeBinding,
            "Item = i64",
            Some("Item = i32"),
        ),
    ];

    for (text, rule, primary_text, related_text) in cases {
        let super::HeaderDefinitionError::Rule(violation) = definition_error(text) else {
            panic!("authored definition failure did not retain its rule")
        };
        assert_eq!(violation.rule(), rule);
        let nocter_syntax::SyntaxOrigin::Token(primary) = violation.primary() else {
            panic!("token-backed definition rule selected a node")
        };
        let primary_start = primary.range().start().get();
        assert_eq!(
            primary_start,
            u32::try_from(text.rfind(primary_text).unwrap()).unwrap()
        );
        match (violation.related(), related_text) {
            (Some(origin), Some(related_text)) => {
                let nocter_syntax::SyntaxOrigin::Token(related) = origin else {
                    panic!("duplicate definition rule selected a related node")
                };
                let related_start = related.range().start().get();
                assert_eq!(
                    related_start,
                    u32::try_from(text.find(related_text).unwrap()).unwrap()
                );
            }
            (None, None) => {}
            _ => panic!("definition rule retained the wrong related subject"),
        }
    }
}

#[test]
fn rejects_invalid_semantic_header_graphs_at_freeze() {
    let super::HeaderDefinitionError::Declaration(empty_enum) = definition_error("enum Empty {}\n")
    else {
        panic!("empty enum did not produce a declaration diagnostic");
    };
    let empty_source = &empty_enum.sources()[0];
    assert_eq!(empty_source.code(), "E0200");
    assert!(empty_source.primary().node().is_some());
    assert_eq!(empty_source.primary().span().range().start().get(), 0);
    assert_eq!(empty_source.primary().span().range().end().get(), 13);
    assert_eq!(empty_source.notes(), []);
    assert!(matches!(
        definition_error(
            "interface Show { pub method &self.show(): i32 }\ninterface Factory { pub method &self.make(): some Show }\n"
        ),
        super::HeaderDefinitionError::Declaration(diagnostics)
            if diagnostics.sources().iter().any(|diagnostic| diagnostic.code() == "E0212")
    ));
    let super::HeaderDefinitionError::Declaration(invalid_result) =
        definition_error("struct Box {}\nconstruct Box {\n    pub func new(): i32 { return }\n}\n")
    else {
        panic!("invalid construction result did not produce a declaration diagnostic");
    };
    let invalid_source = &invalid_result.sources()[0];
    let related = invalid_source.notes()[0].origin();
    let primary_range = invalid_source.primary().span().range();
    let related_range = related.span().range();
    assert_eq!(
        invalid_source.notes()[0].message(),
        "owning construction is declared here"
    );
    assert!(related_range.start() < primary_range.start());
    assert!(primary_range.end() < related_range.end());
    assert!(matches!(
        definition_error("instance str {\n    pub method &self.size(): usize { return 0 }\n}\n"),
        super::HeaderDefinitionError::Declaration(diagnostics)
            if diagnostics.sources().iter().any(|diagnostic| diagnostic.code() == "E0201")
    ));
    assert!(matches!(
        definition_error(
            "interface Pair {\n    pub type First\n    pub type Second\n}\nstruct Box {}\ninstance Box { impl Pair { .First = i32 } }\n"
        ),
        super::HeaderDefinitionError::Declaration(diagnostics)
            if diagnostics.sources().iter().any(|diagnostic| diagnostic.code() == "E0211")
    ));
    for source in [
        "struct Text {}\nconstruct Text {\n    pub literal \"\"(text: i32): Self { return Self {} }\n}\n",
        "struct Text {}\nconstruct Text {\n    pub literal \"\"(text: &str): Self? { return none }\n}\n",
    ] {
        assert!(matches!(
            definition_error(source),
            super::HeaderDefinitionError::Declaration(diagnostics)
                if diagnostics.sources().iter().any(|diagnostic| diagnostic.code() == "E0213")
        ));
    }
}

#[test]
fn declaration_freeze_projects_every_independent_rule_violation() {
    let source = concat!(
        "primitive func first(): usize\n",
        "primitive func second(): usize\n",
        "enum Empty {}\n",
    );
    let super::HeaderDefinitionError::Declaration(diagnostics) = definition_error(source) else {
        panic!("invalid declarations did not produce a validation report");
    };

    assert_eq!(diagnostics.sources().len(), 3);
    assert_eq!(
        diagnostics
            .sources()
            .iter()
            .map(crate::SourceDiagnostic::code)
            .collect::<Vec<_>>(),
        ["E0208", "E0208", "E0200"]
    );
    assert_eq!(
        diagnostics
            .sources()
            .iter()
            .map(|diagnostic| diagnostic.primary().span().range().start().get())
            .collect::<Vec<_>>(),
        [0, 30, 61]
    );
}

#[test]
fn complete_header_identity_is_independent_of_input_order() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "struct Value { item: i32 }\nfunc read(value: &Value): i32 { return 0 }\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let app_tree = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);

    let build = |reverse: bool| {
        let mut packages = vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ];
        let mut modules = vec![
            module("workspace:app", &[], "/app/index.nct", &app_tree),
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ];
        if reverse {
            packages.reverse();
            modules.reverse();
        }
        lower(&sources, packages, modules, vec![], vec![], &prelude)
    };
    let first = build(false);
    let second = build(true);
    assert_eq!(
        format!("{:?}", first.program().packages()),
        format!("{:?}", second.program().packages())
    );
    assert_eq!(
        format!("{:?}", first.program().modules()),
        format!("{:?}", second.program().modules())
    );
    assert_eq!(
        first.program().module_namespaces(),
        second.program().module_namespaces()
    );
    assert_eq!(
        format!("{:?}", first.program().declarations()),
        format!("{:?}", second.program().declarations())
    );
    assert_eq!(
        first.program().types().iter().collect::<Vec<_>>(),
        second.program().types().iter().collect::<Vec<_>>()
    );
    assert_eq!(first.source_index(), second.source_index());
}

#[test]
fn declaration_diagnostic_is_independent_of_compile_unit_input_order() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "enum Empty {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let app_tree = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);

    let build = |reverse: bool| {
        let mut packages = vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ];
        let mut modules = vec![
            module("workspace:app", &[], "/app/index.nct", &app_tree),
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ];
        if reverse {
            packages.reverse();
            modules.reverse();
        }
        let error = try_lower(&sources, packages, modules, vec![], vec![], &prelude).unwrap_err();
        let super::HeaderDefinitionError::Declaration(diagnostic) = error else {
            panic!("empty enum did not produce a declaration diagnostic");
        };
        diagnostic
    };

    assert_eq!(build(false), build(true));
}

#[test]
fn public_index_contract_is_completed_by_one_private_representation_and_body() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let contract_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "see ./record.nct\n",
            "pub struct Box<T>\n",
            "construct Box<T> {\n",
            "    pub func new(value: T): Self\n",
            "}\n",
            "instance Box<T> {\n",
            "    pub operator (&self == other: &Self): bool\n",
            "}\n",
            "pub interface View<T> {\n",
            "    pub method &self.get(): &T from self\n",
            "}\n",
            "instance Box<T> { impl View<T> }\n",
        ),
    );
    let definition_id = add_source(
        &mut sources,
        "/app/record.nct",
        concat!(
            "see ./index.nct\n",
            "struct Box<T> { value: T }\n",
            "construct Box<T> {\n",
            "    func new(value: T): Self { return Box<T> { value: value } }\n",
            "    func hidden(value: T): Self { return Box<T> { value: value } }\n",
            "}\n",
            "instance Box<T> {\n",
            "    operator (&self == other: &Self): bool { return true }\n",
            "}\n",
            "instance Box<T> {\n",
            "    method &self.get(): &T from self { return &self.value }\n",
            "}\n",
        ),
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let contract = parse_source(&sources, contract_id, ParseGoal::SourceFile);
    let definition = parse_source(&sources, definition_id, ParseGoal::SourceFile);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let lowered = lower(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("toolchain:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            ModuleInput::new(
                ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
                vec![
                    ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &contract),
                    ModuleSourceInput::new(
                        "/app/record.nct",
                        ModuleSourceKind::Implementation,
                        &definition,
                    ),
                ],
            ),
            module("toolchain:std", &[], "/std/index.nct", &std_root),
            module(
                "toolchain:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude_tree,
            ),
        ],
        vec![
            source_see(&contract, 0, "/app/record.nct"),
            source_see(&definition, 0, "/app/index.nct"),
        ],
        vec![],
        &prelude,
    );
    let declarations = lowered.program().declarations();

    assert_eq!(declarations.nominal_types().len(), 1);
    assert_eq!(declarations.fields().len(), 1);
    assert_eq!(declarations.constructions().len(), 1);
    assert_eq!(declarations.instances().len(), 1);
    assert_eq!(declarations.interfaces().len(), 1);
    assert_eq!(declarations.interface_implementations().len(), 1);
    assert_eq!(declarations.callables().len(), 5);
    assert_eq!(declarations.bodies().len(), 4);
}
