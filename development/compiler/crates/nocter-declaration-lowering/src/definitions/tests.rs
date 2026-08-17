use nocter_declarations::{
    CallableKind, CallableProvenanceContract, DeclarationRule, ExportedEntity, NominalShape,
};
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use crate::test_support::source_use;

const FULL_HEADER_SOURCE: &str = r#"
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
}

construct Box<T> {
    pub default func new(value: T): Self { return }
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

conform Source<T> for Box<T> where copy T {
    type Item = T
    method &self.get(index: usize): &T from self { return }
}

func values<T>(value: &T): some Source<T, Item = &T> from value { return }
drop Box<T>(&+self) { return }
test headers { return }
"#;
use crate::{
    CompileUnitInput, DefinitionRule, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
    UseResolutionInput, apply_standard_prelude, bind_header_type_syntax,
    collect_declaration_surface, define_declaration_headers, normalize_header_types,
    prepare_authored_imports, prepare_declaration_headers, prepare_generic_binders,
    reserve_declaration_identities,
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

fn lower<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput<'syntax>>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> crate::LoweredDeclarations {
    try_lower(sources, packages, modules, uses, prelude).unwrap()
}

fn try_lower<'syntax>(
    sources: &'syntax SourceMap,
    packages: Vec<PackageInput<'syntax>>,
    modules: Vec<ModuleInput<'syntax>>,
    uses: Vec<UseResolutionInput>,
    prelude: &ModuleIdentity,
) -> Result<crate::LoweredDeclarations, super::HeaderDefinitionError> {
    let input = CompileUnitInput::new(sources, packages, modules, uses);
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_standard_prelude(imports, prelude).unwrap();
    let bound = bind_header_type_syntax(namespaces).unwrap();
    let normalized = normalize_header_types(bound).unwrap();
    define_declaration_headers(normalized)
}

fn definition_error(source_text: &str) -> super::HeaderDefinitionError {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", source_text);
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let app_tree = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let input = CompileUnitInput::new(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("toolchain:std", "std", "/std/nocter.nct", &std_manifest),
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
    );
    let surface = collect_declaration_surface(&input).unwrap();
    let reserved = reserve_declaration_identities(surface).unwrap();
    let headers = prepare_declaration_headers(reserved).unwrap();
    let generics = prepare_generic_binders(headers).unwrap();
    let imports = prepare_authored_imports(generics).unwrap();
    let namespaces = apply_standard_prelude(imports, &prelude).unwrap();
    let bound = bind_header_type_syntax(namespaces).unwrap();
    let normalized = normalize_header_types(bound).unwrap();
    define_declaration_headers(normalized).unwrap_err()
}

#[test]
fn freezes_complete_header_graph_with_exact_leaf_ownership() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(
        &mut sources,
        "/std/prelude/index.nct",
        "pub func shared(): void { return }\n",
    );
    let app_id = add_source(&mut sources, "/app/index.nct", FULL_HEADER_SOURCE);
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let app_tree = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);
    let lowered = lower(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("toolchain:std", "std", "/std/nocter.nct", &std_manifest),
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
        &prelude,
    );
    let program = lowered.program();
    let declarations = program.declarations();

    assert_eq!(declarations.nominal_types().len(), 2);
    assert_eq!(declarations.fields().len(), 1);
    assert_eq!(declarations.variants().len(), 2);
    assert_eq!(declarations.interfaces().len(), 1);
    assert_eq!(declarations.associated_types().len(), 1);
    assert_eq!(declarations.constructions().len(), 1);
    assert_eq!(declarations.instances().len(), 1);
    assert_eq!(declarations.conformances().len(), 1);
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
    assert!(lowered.source_index().len() > declarations.callables().len());
}

#[test]
fn joins_contract_parameters_and_implementation_body_into_one_identity() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use ./implementation\n\npub func select<T>(value: &T): &T from value\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/implementation.nct",
        "func select<T>(value: &T): &T from value { return }\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let root = parse_source(&sources, root_id, ParseGoal::ModuleSource);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::ModuleSource);
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
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("toolchain:std", "std", "/std/nocter.nct", &std_manifest),
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
        vec![source_use(&root, 0, "/app/implementation.nct")],
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
    let error = definition_error(
        "interface Choose {\n    pub method &self.choose(other: &Self): &Self\n}\n",
    );
    assert!(matches!(
        error,
        super::HeaderDefinitionError::Rule(violation)
            if violation.rule() == DefinitionRule::AmbiguousBodylessResultProvenance
    ));
}

#[test]
fn definition_rules_retain_exact_authored_subjects() {
    let cases = [
        (
            "struct Value {}\nconstruct Value {\n    pub default func first(): Self {}\n    pub default func second(): Self {}\n}\n",
            DefinitionRule::DuplicateConstructionDefault,
            "default func second",
            Some("default func first"),
        ),
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
            "interface Source {\n    pub type Item\n}\nstruct Value {}\nconform Source for Value {\n    type Missing = i32\n}\n",
            DefinitionRule::UnknownAssociatedTypeBinding,
            "Missing",
            None,
        ),
        (
            "interface Source {\n    pub type Item\n}\nstruct Value {}\nconform Source for Value {\n    type Item = i32\n    type Item = i64\n}\n",
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
        let nocter_source_index::SyntaxOrigin::Token(primary) = violation.primary() else {
            panic!("token-backed definition rule selected a node")
        };
        let primary_start = primary.range().start().get();
        assert_eq!(
            primary_start,
            u32::try_from(text.rfind(primary_text).unwrap()).unwrap()
        );
        match (violation.related(), related_text) {
            (Some(origin), Some(related_text)) => {
                let nocter_source_index::SyntaxOrigin::Token(related) = origin else {
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
    assert_eq!(empty_enum.rule(), DeclarationRule::EmptyEnum);
    assert_eq!(empty_enum.code(), "E0200");
    assert!(empty_enum.primary().node().is_some());
    assert_eq!(empty_enum.primary().span().range().start().get(), 0);
    assert_eq!(empty_enum.primary().span().range().end().get(), 13);
    assert_eq!(empty_enum.related(), None);
    assert!(matches!(
        definition_error(
            "interface Show { pub method &self.show(): i32 }\ninterface Factory { pub method &self.make(): some Show }\n"
        ),
        super::HeaderDefinitionError::Declaration(diagnostic)
            if diagnostic.rule() == DeclarationRule::InvalidOpaqueResult
    ));
    let super::HeaderDefinitionError::Declaration(invalid_result) =
        definition_error("struct Box {}\nconstruct Box {\n    pub func new(): i32 { return }\n}\n")
    else {
        panic!("invalid construction result did not produce a declaration diagnostic");
    };
    assert_eq!(
        invalid_result.rule(),
        DeclarationRule::InvalidConstructionResult
    );
    let related = invalid_result.related().unwrap();
    let primary_range = invalid_result.primary().span().range();
    let related_range = related.span().range();
    assert_eq!(
        invalid_result.related_message(),
        Some("owning construction is declared here")
    );
    assert!(related_range.start() < primary_range.start());
    assert!(primary_range.end() < related_range.end());
    assert!(matches!(
        definition_error("instance str {\n    pub method &self.size(): usize { return 0 }\n}\n"),
        super::HeaderDefinitionError::Declaration(diagnostic)
            if diagnostic.rule() == DeclarationRule::InvalidInherentAttachment
    ));
    assert!(matches!(
        definition_error(
            "interface Pair {\n    pub type First\n    pub type Second\n}\nstruct Box {}\nconform Pair for Box {\n    type First = i32\n}\n"
        ),
        super::HeaderDefinitionError::Declaration(diagnostic)
            if diagnostic.rule() == DeclarationRule::IncompleteAssociatedTypes
    ));
    for source in [
        "struct Text {}\nconstruct Text {\n    pub literal \"\"(text: i32): Self { return Self {} }\n}\n",
        "struct Text {}\nconstruct Text {\n    pub literal \"\"(text: &str): Self? { return none }\n}\n",
    ] {
        assert!(matches!(
            definition_error(source),
            super::HeaderDefinitionError::Declaration(diagnostic)
                if diagnostic.rule() == DeclarationRule::InvalidLiteralSignature
                    && diagnostic.code() == "E0213"
        ));
    }
}

#[test]
fn complete_header_identity_is_independent_of_input_order() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "struct Value { item: i32 }\nfunc read(value: &Value): i32 { return 0 }\n",
    );
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let app_tree = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);

    let build = |reverse: bool| {
        let mut packages = vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("toolchain:std", "std", "/std/nocter.nct", &std_manifest),
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
        lower(&sources, packages, modules, vec![], &prelude)
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
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_id = add_source(&mut sources, "/app/index.nct", "enum Empty {}\n");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::ModuleSource);
    let prelude_tree = parse_source(&sources, prelude_id, ParseGoal::ModuleSource);
    let app_tree = parse_source(&sources, app_id, ParseGoal::ModuleSource);
    let prelude = ModuleIdentity::new(PackageIdentity::new("toolchain:std"), ["prelude"]);

    let build = |reverse: bool| {
        let mut packages = vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("toolchain:std", "std", "/std/nocter.nct", &std_manifest),
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
        let error = try_lower(&sources, packages, modules, vec![], &prelude).unwrap_err();
        let super::HeaderDefinitionError::Declaration(diagnostic) = error else {
            panic!("empty enum did not produce a declaration diagnostic");
        };
        diagnostic
    };

    assert_eq!(build(false), build(true));
}
