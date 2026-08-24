use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

use super::{DeclarationContractError, analyze_declaration_contracts};
use crate::test_support::source_include;
use crate::{
    CompileUnitInput, IncludeResolutionInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
    SurfaceDeclarationId, collect_declaration_surface,
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

fn surface<'syntax>(
    sources: &'syntax SourceMap,
    manifest: &'syntax SyntaxTree,
    module_sources: Vec<ModuleSourceInput<'syntax>>,
    include_resolutions: Vec<IncludeResolutionInput>,
) -> crate::DeclarationSurface<'syntax> {
    let package = PackageInput::new(
        PackageIdentity::new("workspace:app"),
        "app",
        PackageMode::Declared,
        Some(PackageDeclarationInput::new("/app/nocter.nct", manifest)),
    );
    let module = ModuleInput::new(
        ModuleIdentity::new(PackageIdentity::new("workspace:app"), Vec::<&str>::new()),
        module_sources,
    );
    collect_declaration_surface(
        &CompileUnitInput::new(
            nocter_model::CompilationTarget::Arm64Darwin,
            sources,
            vec![package],
            vec![module],
            Vec::new(),
        )
        .with_include_resolutions(include_resolutions),
    )
    .unwrap()
}

#[test]
fn exact_contracts_and_bodies_share_the_contract_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./parse.nct\n\npub func parse(\n    text: &str\n): usize\n\ninstance Text {\n    pub method &self.len(): usize\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 0 }\n\ninstance Text {\n    method &self.len(): usize { 0 }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/parse.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/parse.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(3)),
        SurfaceDeclarationId::from_index(0)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(2)
    );
}

#[test]
fn constant_contract_joins_exactly_one_reciprocal_private_initializer() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./limits.nct\npub const buffer_size: usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/limits.nct",
        "include ./index.nct\nconst buffer_size: usize = 4096\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/limits.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/limits.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(1)),
        SurfaceDeclarationId::from_index(0)
    );
}

#[test]
fn constant_contract_rejects_a_different_initializer_header() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./limits.nct\npub const buffer_size: usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/limits.nct",
        "include ./index.nct\nconst buffer_size: u32 = 4096\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/limits.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/limits.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::MismatchedConstantInitializer { .. })
    ));
}

#[test]
fn nested_opaque_results_share_their_callable_contract_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./source.nct\n",
            "pub interface Source { pub method &+self.next(): i32? }\n",
            "pub func source(): some Source\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/source.nct",
        "include ./index.nct\nfunc source(): some Source { return source() }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/source.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/source.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();
    let mut contract_opaque = Vec::new();
    let mut body_opaque = Vec::new();
    for (index, declaration) in surface.declarations().iter().copied().enumerate() {
        if declaration.kind() != crate::SurfaceDeclarationKind::OpaqueType {
            continue;
        }
        let id = SurfaceDeclarationId::from_index(index);
        match surface.sources()[declaration.source().index()].kind() {
            ModuleSourceKind::Root => contract_opaque.push(id),
            ModuleSourceKind::Implementation => body_opaque.push(id),
            ModuleSourceKind::SingleFile => unreachable!(),
        }
    }
    assert_eq!(contract_opaque.len(), body_opaque.len());
    assert!(!contract_opaque.is_empty());
    for (contract, body) in contract_opaque.into_iter().zip(body_opaque) {
        assert_eq!(contracts.representative(body), contract);
    }
}

#[test]
fn same_callable_label_with_a_different_header_is_a_mismatch() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./parse.nct\n\npub func parse(text: &str): usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/parse.nct",
        "include ./index.nct\n\nfunc parse(text: usize): usize { text }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/parse.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/parse.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::MismatchedBody { .. })
    ));
}

#[test]
fn duplicate_matching_bodies_are_rejected_independent_of_source_order() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./a.nct\ninclude ./b.nct\n\npub func parse(text: &str): usize\n",
    );
    let first_id = add_source(
        &mut sources,
        "/app/a.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 1 }\n",
    );
    let second_id = add_source(
        &mut sources,
        "/app/b.nct",
        "include ./index.nct\n\nfunc parse(text: &str): usize { 2 }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let first = parse_source(&sources, first_id, ParseGoal::SourceFile);
    let second = parse_source(&sources, second_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/b.nct", ModuleSourceKind::Implementation, &second),
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new("/app/a.nct", ModuleSourceKind::Implementation, &first),
        ],
        vec![
            source_include(&root, 0, "/app/a.nct"),
            source_include(&root, 1, "/app/b.nct"),
            source_include(&first, 0, "/app/index.nct"),
            source_include(&second, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::DuplicateBody { .. })
    ));
}

#[test]
fn body_omission_is_not_a_general_callable_form() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(&mut sources, "/app/index.nct", "func unfinished(): void\n");
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
        Vec::new(),
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::InvalidBodyOmission(_))
    ));
}

#[test]
fn private_conformance_definition_cannot_repeat_associated_type_bindings() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./value.nct\n",
            "pub interface Source { pub type Item }\n",
            "pub struct Value {}\n",
            "conform Source for Value { type Item = i32 }\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        concat!(
            "include ./index.nct\n",
            "conform Source for Value { type Item = i32 }\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::InvalidConformanceSplit(_))
    ));
}

#[test]
fn coercion_bodies_use_the_same_contract_joining_rule() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./view.nct\n\ninstance Text {\n    pub coerce &self as &str\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/view.nct",
        "include ./index.nct\n\ninstance Text {\n    coerce &self as &str { self }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/view.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/view.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(3)),
        SurfaceDeclarationId::from_index(1)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(2)),
        SurfaceDeclarationId::from_index(0)
    );
}

#[test]
fn construction_body_omits_visibility_but_repeats_default_and_keeps_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./value.nct\n\nstruct Value { value: usize }\n\nconstruct Value {\n    pub default func new(): Self\n}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        "include ./index.nct\n\nconstruct Value {\n    default func new(): Self { Value { value: 0 } }\n}\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    assert!(!implementation.has_errors());
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(3)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(4)),
        SurfaceDeclarationId::from_index(2)
    );
}

#[test]
fn opaque_nominal_contract_and_private_representation_share_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./string.nct\n\npub struct String\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/string.nct",
        "include ./index.nct\n\nstruct String { len: usize }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/string.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/string.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(1)),
        SurfaceDeclarationId::from_index(0)
    );
}

#[test]
fn implementation_source_cannot_add_program_wide_conformance() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./value.nct\n",
            "pub interface Read { pub method &self.read(): usize }\n",
            "pub struct Value {}\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        concat!(
            "include ./index.nct\n",
            "conform Read for Value {\n",
            "    method &self.read(): usize { return 0 }\n",
            "}\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::UncontractedConformance(_))
    ));
}

#[test]
fn conformance_head_joins_private_methods_without_repeating_interface_signatures() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./value.nct\n",
            "pub interface Read { pub method &self.read(): usize }\n",
            "pub struct Value {}\n",
            "conform Read for Value {}\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        concat!(
            "include ./index.nct\n",
            "conform Read for Value {\n",
            "    method &self.read(): usize { return 0 }\n",
            "}\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(4)),
        SurfaceDeclarationId::from_index(3)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(5)),
        SurfaceDeclarationId::from_index(5)
    );
}

#[test]
fn conformance_contract_cannot_repeat_an_interface_method_signature() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./value.nct\n",
            "pub interface Read { pub method &self.read(): usize }\n",
            "pub struct Value {}\n",
            "conform Read for Value { method &self.read(): usize }\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/value.nct",
        concat!(
            "include ./index.nct\n",
            "conform Read for Value {\n",
            "    method &self.read(): usize { return 0 }\n",
            "}\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/value.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/value.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::InvalidConformanceSplit(_))
    ));
}

#[test]
fn interface_default_contract_and_private_body_share_one_identity() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "include ./defaults.nct\n",
            "pub interface Source {\n",
            "    pub method &+self.next(): i32?\n",
            "    pub default method self.count(): usize\n",
            "}\n",
        ),
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/defaults.nct",
        concat!(
            "include ./index.nct\n",
            "interface Source {\n",
            "    default method self.count(): usize { return 0 }\n",
            "}\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/defaults.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/defaults.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();

    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(4)),
        SurfaceDeclarationId::from_index(2)
    );
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(3)),
        SurfaceDeclarationId::from_index(0)
    );
}

#[test]
fn interface_requirement_does_not_request_an_implementation_body() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "pub interface Source { pub method &+self.next(): i32? }\n",
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![ModuleSourceInput::new(
            "/app/index.nct",
            ModuleSourceKind::Root,
            &root,
        )],
        Vec::new(),
    );

    analyze_declaration_contracts(&surface).unwrap();
}

#[test]
fn uncontracted_interface_default_body_is_rejected() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./defaults.nct\npub interface Source {}\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/defaults.nct",
        concat!(
            "include ./index.nct\n",
            "interface Source {\n",
            "    default method self.count(): usize { return 0 }\n",
            "}\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/defaults.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/defaults.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    assert!(matches!(
        analyze_declaration_contracts(&surface),
        Err(DeclarationContractError::UncontractedInterfaceDefault(_))
    ));
}

#[test]
fn selected_target_body_completes_one_target_independent_contract() {
    let mut sources = SourceMap::new();
    let manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let root_id = add_source(
        &mut sources,
        "/app/index.nct",
        "include ./platform.nct\npub func process_id(): usize\n",
    );
    let implementation_id = add_source(
        &mut sources,
        "/app/platform.nct",
        concat!(
            "include ./index.nct\n",
            "#target: \"arm64-darwin\"\n",
            "func process_id(): usize { return 1 }\n",
        ),
    );
    let manifest = parse_source(&sources, manifest_id, ParseGoal::PackageFile);
    let root = parse_source(&sources, root_id, ParseGoal::SourceFile);
    let implementation = parse_source(&sources, implementation_id, ParseGoal::SourceFile);
    let surface = surface(
        &sources,
        &manifest,
        vec![
            ModuleSourceInput::new("/app/index.nct", ModuleSourceKind::Root, &root),
            ModuleSourceInput::new(
                "/app/platform.nct",
                ModuleSourceKind::Implementation,
                &implementation,
            ),
        ],
        vec![
            source_include(&root, 0, "/app/platform.nct"),
            source_include(&implementation, 0, "/app/index.nct"),
        ],
    );

    let contracts = analyze_declaration_contracts(&surface).unwrap();
    assert_eq!(
        contracts.representative(SurfaceDeclarationId::from_index(1)),
        SurfaceDeclarationId::from_index(0)
    );
}
