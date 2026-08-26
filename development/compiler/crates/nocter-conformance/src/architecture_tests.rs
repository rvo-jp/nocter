use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("conformance crate is inside the compiler workspace")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    fs::read_to_string(workspace().join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

fn production_dependencies(crate_name: &str) -> BTreeSet<String> {
    let manifest = source(&format!("crates/{crate_name}/Cargo.toml"));
    let Some((_, dependencies)) = manifest.split_once("[dependencies]\n") else {
        return BTreeSet::new();
    };
    dependencies
        .split("\n[")
        .next()
        .expect("dependency section exists")
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
        .collect()
}

fn production_dependency_closure(crate_name: &str) -> BTreeSet<String> {
    fn visit(crate_name: &str, closure: &mut BTreeSet<String>) {
        for dependency in production_dependencies(crate_name) {
            if closure.insert(dependency.clone())
                && workspace()
                    .join("crates")
                    .join(&dependency)
                    .join("Cargo.toml")
                    .is_file()
            {
                visit(&dependency, closure);
            }
        }
    }

    let mut closure = BTreeSet::new();
    visit(crate_name, &mut closure);
    closure
}

#[test]
fn core_program_layers_keep_the_reviewed_dependency_direction() {
    let expected = [
        ("nocter-language", &[][..]),
        ("nocter-model", &["nocter-language"][..]),
        ("nocter-declarations", &["nocter-model"][..]),
        (
            "nocter-target-program",
            &[
                "nocter-checking",
                "nocter-declarations",
                "nocter-model",
                "nocter-runtime-contract",
            ][..],
        ),
        (
            "nocter-mir",
            &[
                "nocter-checking",
                "nocter-model",
                "nocter-runtime-contract",
                "nocter-target-program",
            ][..],
        ),
        (
            "nocter-machine",
            &["nocter-mir", "nocter-model", "nocter-runtime-contract"][..],
        ),
        (
            "nocter-arm64",
            &["nocter-machine", "nocter-runtime-contract"][..],
        ),
        ("nocter-macho", &["nocter-arm64", "nocter-hash"][..]),
    ];
    for (crate_name, allowed) in expected {
        let actual = production_dependencies(crate_name);
        let allowed = allowed
            .iter()
            .map(|dependency| (*dependency).to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, allowed,
            "review production dependencies for {crate_name}"
        );
    }
}

#[test]
fn semantic_editor_stack_does_not_inherit_native_backend_layers() {
    let forbidden = [
        "nocter-arm64",
        "nocter-machine",
        "nocter-macho",
        "nocter-mir",
        "nocter-native-session",
    ];
    for crate_name in [
        "nocter-session",
        "nocter-analysis",
        "nocter-language-server",
    ] {
        let closure = production_dependency_closure(crate_name);
        let inherited = forbidden
            .iter()
            .filter(|dependency| closure.contains(**dependency))
            .collect::<Vec<_>>();
        assert!(
            inherited.is_empty(),
            "{crate_name} inherits native backend crates: {inherited:?}"
        );
    }
}

#[test]
fn semantic_authorities_do_not_read_editor_projection_contracts() {
    let standard = source("crates/nocter-checking/src/standard_semantics.rs");
    assert!(!standard.contains("nocter_source_index"));
    assert!(!standard.contains("FrontendBindings"));

    let toolchain = source("crates/nocter-declaration-lowering/src/toolchain.rs");
    assert!(!toolchain.contains("nocter_source_index"));
    assert!(!toolchain.contains("FrontendBindings"));

    let target = source("crates/nocter-target-program/src/program.rs");
    assert!(!target.contains("validate_package_targets"));

    for path in [
        "crates/nocter-declaration-lowering/src/surface.rs",
        "crates/nocter-declaration-lowering/src/reservation.rs",
        "crates/nocter-declaration-lowering/src/definitions/mod.rs",
    ] {
        assert!(!source(path).contains("TargetSelection::prepare"));
    }
}

#[test]
fn toolchain_roles_are_located_without_discovery_owned_syntax_authority() {
    let discovery = source("crates/nocter-discovery/src/graph.rs");
    assert!(!discovery.contains("declaration_name_token"));
    assert!(!discovery.contains("resolve_standard_role"));
    assert!(!discovery.contains("resolve_primitive_role"));
    assert!(!discovery.contains("resolve_builtin_type"));

    let compile_input = source("crates/nocter-compile-input/src/lib.rs");
    let locator_contract = compile_input
        .split_once("pub struct StandardRoleLocator")
        .expect("standard role locator exists")
        .1
        .split_once("pub struct StructuralAttachmentInput")
        .expect("locator contract ends before structural attachments")
        .0;
    assert!(!locator_contract.contains("SyntaxToken"));

    let lowering = source("crates/nocter-declaration-lowering/src/toolchain.rs");
    assert!(lowering.contains("SurfaceDeclarationId"));
    assert!(!lowering.contains("declaration_name_token"));

    let reservation = source("crates/nocter-declaration-lowering/src/reservation.rs");
    assert!(!reservation.contains("declaration.name() == Some(role.declaration())"));
}

#[test]
fn body_recovery_promotes_or_discards_one_isolated_transaction() {
    let pipeline = source("crates/nocter-checking/src/body_check/pipeline.rs");
    assert!(pipeline.contains("struct BodySemanticTransaction"));
    assert!(pipeline.contains("transaction.commit(types, copyabilities, closures)"));
    assert!(pipeline.contains("BodyAttemptFailure::Transactional"));
    assert!(!pipeline.contains("transaction: Option<Box<BodySemanticTransaction>>"));
    assert!(!pipeline.contains("recovery body attempt must own its isolated transaction"));
    assert!(!pipeline.contains("*types = type_checkpoint"));
    assert!(!pipeline.contains("*copyabilities = copyability_checkpoint"));
    assert!(!pipeline.contains("prepared.types.clone()"));
    assert!(!pipeline.contains("prepared.copyabilities.clone()"));
}

#[test]
fn frozen_cross_phase_contracts_are_consumed_without_reconstruction() {
    let provenance = source("crates/nocter-checking/src/provenance/analysis.rs");
    assert!(provenance.contains(".selected_input(origin)"));
    assert!(!provenance.contains("fn map_origin_positions("));

    let mir_environment = source("crates/nocter-mir/src/validation_environment.rs");
    assert!(mir_environment.contains("fn type_representation("));
    assert!(!mir_environment.contains("fn nominal_type("));
    assert!(!mir_environment.contains("fn field("));
    assert!(!mir_environment.contains("fn variant("));
    assert!(!mir_environment.contains("fn parameter("));

    let mir_manifest = production_dependencies("nocter-mir");
    assert!(!mir_manifest.contains("nocter-declarations"));
}

#[test]
fn dense_identity_domains_do_not_retain_parallel_lookup_authorities() {
    let machine = source("crates/nocter-machine/src/lower.rs");
    assert!(machine.contains("MachineFunctionDomain::new(&linkage)"));
    assert!(!machine.contains("struct FunctionDomains"));
    assert!(!machine.contains("assign_function_domains"));

    let program = source("crates/nocter-machine/src/program.rs");
    assert!(!program.contains("functions_by_linkage"));

    let executable = source("crates/nocter-target-program/src/executable.rs");
    assert!(executable.contains("binary_search_by"));
    assert!(!executable.contains("closure_layouts: BTreeMap"));
}

#[test]
fn mir_program_builder_is_the_single_semantic_validation_owner() {
    let function_builder = source("crates/nocter-mir/src/builder.rs");
    assert!(!function_builder.contains("validate_function("));
    assert!(!function_builder.contains("MirValidationEnvironment"));

    let program = source("crates/nocter-mir/src/program.rs");
    let define = program
        .split_once("pub fn define(")
        .expect("MIR program function definition boundary exists")
        .1
        .split_once("pub fn finish(")
        .expect("function definition ends before program freezing")
        .0;
    assert_eq!(define.matches("validate_function(").count(), 1);
}

#[test]
fn prepared_semantic_program_excludes_syntax_owned_body_state() {
    let preparation = source("crates/nocter-checking/src/preparation.rs");
    let semantic_contract = preparation
        .split_once("pub struct PreparedSemanticProgram")
        .expect("prepared semantic program exists")
        .1
        .split_once("impl PreparedSemanticProgram")
        .expect("prepared semantic fields precede its implementation")
        .0;
    assert!(!semantic_contract.contains("ResolvedBodyNames"));
    assert!(!semantic_contract.contains("BodySourceCatalog"));

    let prepared_checking = preparation
        .split_once("pub struct PreparedChecking<'syntax>")
        .expect("prepared checking exists")
        .1
        .split_once("pub struct PreparedBodyAnalysis")
        .expect("prepared checking fields precede analysis wrapper")
        .0;
    assert!(prepared_checking.contains("semantic: PreparedSemanticProgram"));
}

#[test]
fn reservation_owns_bidirectional_surface_entity_identity() {
    let reservation = source("crates/nocter-declaration-lowering/src/reservation.rs");
    assert!(reservation.contains("struct ReservedEntityIndex"));
    assert!(
        reservation.contains("representatives: BTreeMap<ReservedEntity, SurfaceDeclarationId>")
    );

    let type_context = source("crates/nocter-declaration-lowering/src/types/context.rs");
    assert!(type_context.contains(".declaration_for_entity(entity)"));
    assert!(!type_context.contains(".position(|candidate| *candidate == Some(entity))"));

    let normalization = source("crates/nocter-declaration-lowering/src/types/normalization.rs");
    assert!(!normalization.contains(".position(|entity|"));

    let topology = source("crates/nocter-declaration-lowering/src/topology.rs");
    assert!(topology.contains("let mut source_index = SourceIndexBuilder::new()"));
    assert!(!topology.contains("let _ = frontend_bindings"));
}

#[test]
fn named_associated_types_are_indexed_once_at_header_resolution() {
    let headers = source("crates/nocter-declaration-lowering/src/headers/mod.rs");
    assert!(headers.contains("struct AssociatedTypeIndex"));
    assert!(headers.contains("fn index_associated_types("));

    let opaque = source("crates/nocter-declaration-lowering/src/types/opaque.rs");
    assert!(opaque.contains(".associated_type(interface, name)"));
    assert!(!opaque.contains(".find_map(|(index, entity)|"));

    let preparation =
        source("crates/nocter-declaration-lowering/src/types/normalization/preparation.rs");
    assert!(preparation.contains(".associated_types()"));
    assert!(!preparation.contains("fn collect_associated("));

    let definitions =
        source("crates/nocter-declaration-lowering/src/definitions/declarations/mod.rs");
    assert!(definitions.contains(".associated_type(interface.interface(), name)"));
    assert!(!definitions.contains("fn associated_by_name("));
}

#[test]
fn executable_freezing_preserves_reserved_item_identities() {
    let build = source("crates/nocter-target-program/src/executable/build.rs");
    let freeze = build
        .split_once("fn freeze_items(")
        .expect("executable item freeze exists")
        .1
        .split_once("fn freeze_body(")
        .expect("item freeze ends before body freeze")
        .0;
    assert!(freeze.contains("key_arena.try_finish_with"));
    assert!(!freeze.contains("let _ = key_arena.finish()"));
    assert!(!freeze.contains("let mut items = ArenaBuilder::new()"));
}

#[test]
fn construction_selection_consumes_its_frozen_surface_contract() {
    let surfaces = source("crates/nocter-checking/src/construction_surfaces.rs");
    assert!(surfaces.contains("struct VisibleEntry<I>"));
    assert!(surfaces.contains("member_order: Box<[VisibleEntry<CallableId>]>"));
    assert!(!surfaces.contains("fn surface_member_is_indexed("));
    assert!(!surfaces.contains("fn member_is_visible("));
    assert!(!surfaces.contains("fn field_is_visible("));

    let constructions = source("crates/nocter-checking/src/body_check/checker/constructions.rs");
    assert!(!constructions.contains("callable.kind() != CallableKind::ConstructionFunction"));
    assert!(!constructions.contains("callable.owner() != CallableOwner::Construction"));
}

#[test]
fn editor_mutations_select_semantic_identities_instead_of_standard_api_spellings() {
    let action = source("crates/nocter-analysis/src/code_actions/conformance.rs");
    assert!(!action.contains("std/process.abort"));
    assert!(!action.contains("completion.label() != \"abort\""));
    assert!(action.contains("StandardDeclarationRole::ProcessAbort"));
    assert!(action.contains(".standard_semantics()"));
    assert!(action.contains("SemanticEntity::Callable(terminator)"));
}

#[test]
fn builtin_type_vocabulary_has_one_definition() {
    let roots = [
        "crates/nocter-language/src/lib.rs",
        "crates/nocter-model/src/type_store.rs",
        "crates/nocter-syntax/src/token.rs",
    ];
    let definitions = roots
        .iter()
        .map(|path| source(path).matches("pub enum BuiltinType").count())
        .sum::<usize>();
    assert_eq!(definitions, 1);
}
