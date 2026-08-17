use nocter_checking::{
    GenericArgument, GenericArguments, check_prepared_program, prepare_program_checking,
};
use nocter_declaration_lowering::{
    CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput, ModuleSourceKind,
    PackageDeclarationInput, PackageIdentity, PackageInput, PackageMode,
    PackageTargetResolutionInput, lower_compile_unit_declarations,
};
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_model::{BuiltinType, CompilationTarget, TypeKind};
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

use super::{TargetProgram, TargetProgramError};
use crate::{
    CallableInstanceKey, CallableInstanceKeyError, EntrySelectionError, PrimitiveBinding,
    PrimitiveContractRule, PrimitiveRegistry, PrimitiveRegistryValidationError, PrimitiveRole,
    ProcessSuccessType, ToolchainSnapshot, select_executable_entry, select_test_target,
};

const ERROR_SOURCE: &str = "pub(/) primitive new_error(code: &str, message: &str): error\n";
const MEM_SOURCE: &str = "\
pub(/) primitive current_allocator_state(): usize
pub(/) primitive current_allocator_kind(): usize
pub(/) primitive allocation_abort_raw(): never
";
const PTR_SOURCE: &str = "\
pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
pub primitive from_ref_mut<T>(value: &+T): *T
pub(/) primitive from_addr<T>(address: usize): *T
pub(/) primitive pointee_size<T>(pointer: *T): usize
pub(/) primitive pointee_align<T>(pointer: *T): usize
pub(/) primitive copy_str_to_ptr(destination: *u8, offset: usize, text: &str): void
pub(/) primitive copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
pub(/) primitive store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
pub(/) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
pub(/) primitive drop_value_at_ptr<T>(pointer: *T, offset: usize): void
pub(/) primitive take_value_at_ptr<T>(pointer: *T, offset: usize): T
pub(/) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
pub(/) primitive slice_from_raw_parts(pointer: *u8, len: usize): &[u8]
pub(/) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
pub(/) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
pub(/) primitive slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
";
const STRING_SOURCE: &str = "\
pub(/) primitive bytes_from_str(value: &str): &[u8]
pub(/) primitive str_subview_unchecked(text: &str, start: usize, len: usize): &str from text
";
const SLICE_SOURCE: &str = "\
pub(/) primitive slice_len_raw<T>(value: &[T]): usize
pub(/) primitive slice_ptr_addr_raw<T>(value: &[T]): usize
";
const STR_SOURCE: &str = "\
pub(/) primitive str_len_raw(value: &str): usize
pub(/) primitive str_ptr_addr_raw(value: &str): usize
";
const PROCESS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) primitive exit_raw(code: i32): never
#target: \"arm64-darwin\"
pub(/) primitive arg_count_raw(): usize
#target: \"arm64-darwin\"
pub(/) primitive arg_raw(index: usize): &str
#target: \"arm64-darwin\"
pub(/) primitive env_count_raw(): usize
#target: \"arm64-darwin\"
pub(/) primitive env_name_raw(index: usize): &str
#target: \"arm64-darwin\"
pub(/) primitive env_value_raw(index: usize): &str
";
const IO_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) primitive open_read_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive create_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive append_raw(path: *u8): i32!
#target: \"arm64-darwin\"
pub(/) primitive write_text_raw(fd: i32, text: &str): void!
#target: \"arm64-darwin\"
pub(/) primitive write_bytes_raw(fd: i32, bytes: &[u8]): void!
#target: \"arm64-darwin\"
pub(/) primitive read_bytes_raw(fd: i32, buffer: &+[u8]): usize!
#target: \"arm64-darwin\"
pub(/) primitive close_fd_raw(fd: i32): void
";
const INTERNAL_OS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}
#target: \"arm64-darwin\"
pub(/) primitive syscall0(number: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall1(number: usize, a0: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive trap(): never
#target: \"arm64-darwin\"
pub(/) primitive unreachable(): never
";

struct FixtureModule {
    path: Box<[Box<str>]>,
    source_path: Box<str>,
    syntax: SyntaxTree,
}

struct Fixture {
    sources: SourceMap,
    app_manifest: Option<SyntaxTree>,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    standard: SyntaxTree,
    prelude: SyntaxTree,
    modules: Vec<FixtureModule>,
}

impl Fixture {
    fn new() -> Self {
        Self::with_app("func main(): i32! { return 0 }\n")
    }

    fn with_app(app_source: &str) -> Self {
        Self::build(app_source, None)
    }

    fn with_tests(app_source: &str) -> Self {
        Self::build(
            app_source,
            Some("#test: { name: \"unit\", module: \".\", }\n"),
        )
    }

    fn build(app_source: &str, app_manifest_source: Option<&str>) -> Self {
        let mut sources = SourceMap::new();
        let std_manifest = add_parsed(&mut sources, "/std/nocter.nct", "", ParseGoal::PackageFile);
        let app_manifest = app_manifest_source.map(|text| {
            add_parsed(
                &mut sources,
                "/app/nocter.nct",
                text,
                ParseGoal::PackageFile,
            )
        });
        let app_path = if app_manifest.is_some() {
            "/app/index.nct"
        } else {
            "/tmp/app.nct"
        };
        let app = add_parsed(&mut sources, app_path, app_source, ParseGoal::ModuleSource);
        let standard = add_parsed(&mut sources, "/std/index.nct", "", ParseGoal::ModuleSource);
        let prelude = add_parsed(
            &mut sources,
            "/std/prelude/index.nct",
            "",
            ParseGoal::ModuleSource,
        );
        let modules = [
            (&["error"][..], ERROR_SOURCE),
            (&["mem"][..], MEM_SOURCE),
            (&["ptr"][..], PTR_SOURCE),
            (&["string"][..], STRING_SOURCE),
            (&["slice"][..], SLICE_SOURCE),
            (&["str"][..], STR_SOURCE),
            (&["process"][..], PROCESS_SOURCE),
            (&["io"][..], IO_SOURCE),
            (&["internal", "os"][..], INTERNAL_OS_SOURCE),
        ]
        .into_iter()
        .map(|(path, text)| {
            let source_path = format!("/std/{}/index.nct", path.join("/"));
            let tree = add_parsed(&mut sources, &source_path, text, ParseGoal::ModuleSource);
            FixtureModule {
                path: path
                    .iter()
                    .map(|segment| Box::<str>::from(*segment))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                source_path: source_path.into_boxed_str(),
                syntax: tree,
            }
        })
        .collect();
        Self {
            sources,
            app_manifest,
            std_manifest,
            app,
            standard,
            prelude,
            modules,
        }
    }

    fn input(&self) -> (CompileUnitInput<'_>, ModuleIdentity) {
        let app_package = PackageIdentity::new("workspace:app");
        let standard_package = PackageIdentity::new("toolchain:std");
        let app_declaration = self
            .app_manifest
            .as_ref()
            .map(|manifest| PackageDeclarationInput::new("/app/nocter.nct", manifest));
        let packages = vec![
            PackageInput::new(
                app_package.clone(),
                "app",
                if app_declaration.is_some() {
                    PackageMode::Declared
                } else {
                    PackageMode::SingleFile
                },
                app_declaration,
            ),
            package(
                standard_package.clone(),
                "std",
                "/std/nocter.nct",
                &self.std_manifest,
            ),
        ];
        let mut modules = vec![
            ModuleInput::new(
                ModuleIdentity::new(app_package.clone(), Vec::<&str>::new()),
                vec![ModuleSourceInput::new(
                    if self.app_manifest.is_some() {
                        "/app/index.nct"
                    } else {
                        "/tmp/app.nct"
                    },
                    if self.app_manifest.is_some() {
                        ModuleSourceKind::Root
                    } else {
                        ModuleSourceKind::SingleFile
                    },
                    &self.app,
                )],
            ),
            module(
                standard_package.clone(),
                &[],
                "/std/index.nct",
                &self.standard,
            ),
            module(
                standard_package.clone(),
                &["prelude"],
                "/std/prelude/index.nct",
                &self.prelude,
            ),
        ];
        modules.extend(self.modules.iter().map(|fixture| {
            let segments = fixture.path.iter().map(AsRef::as_ref).collect::<Vec<_>>();
            module(
                standard_package.clone(),
                &segments,
                &fixture.source_path,
                &fixture.syntax,
            )
        }));
        let prelude = ModuleIdentity::new(standard_package, ["prelude"]);
        let mut input = CompileUnitInput::new(
            CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            Vec::new(),
        );
        if let Some(manifest) = &self.app_manifest {
            let declaration = manifest
                .children(manifest.root_id())
                .iter()
                .find_map(|element| match element {
                    SyntaxElement::Node(node)
                        if manifest
                            .node(*node)
                            .is_some_and(|node| node.kind() == NodeKind::PackageDirective) =>
                    {
                        Some(*node)
                    }
                    SyntaxElement::Node(_)
                    | SyntaxElement::Token(_)
                    | SyntaxElement::Missing(_) => None,
                })
                .expect("test fixture manifest has no target directive");
            input = input.with_package_target_resolutions(vec![PackageTargetResolutionInput::new(
                declaration,
                ModuleIdentity::new(app_package, Vec::<&str>::new()),
            )]);
        }
        (input, prelude)
    }
}

#[test]
fn complete_closed_registry_constructs_a_target_program() {
    let fixture = Fixture::new();
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
            .unwrap();
    let target = TargetProgram::build(checked, snapshot).unwrap();
    assert_eq!(
        target.checked().graph().target(),
        CompilationTarget::Arm64Darwin
    );
    assert_eq!(target.toolchain().primitives().bindings().len(), 49);
}

#[test]
fn executable_entry_accepts_exactly_the_six_process_result_forms() {
    for (source, expected_success, expected_fallible) in [
        (
            "func main(): void { return }\n",
            ProcessSuccessType::Void,
            false,
        ),
        (
            "func main(): void! { return }\n",
            ProcessSuccessType::Void,
            true,
        ),
        (
            "func main(): i32 { return 0 }\n",
            ProcessSuccessType::I32,
            false,
        ),
        (
            "func main(): i32! { return 0 }\n",
            ProcessSuccessType::I32,
            true,
        ),
        (
            "func main(): usize { return 0 }\n",
            ProcessSuccessType::Usize,
            false,
        ),
        (
            "func main(): usize! { return 0 }\n",
            ProcessSuccessType::Usize,
            true,
        ),
    ] {
        let target = build_target_program(&Fixture::with_app(source));
        let (target_id, _) = target
            .checked()
            .graph()
            .package_targets()
            .iter()
            .next()
            .unwrap();
        let entry = select_executable_entry(&target, target_id).unwrap();
        assert_eq!(entry.process_result().success(), expected_success);
        assert_eq!(entry.process_result().is_fallible(), expected_fallible);
        assert_eq!(entry.target(), target_id);
        assert_eq!(
            target
                .checked()
                .graph()
                .declarations()
                .callables()
                .get(entry.callable())
                .and_then(nocter_declarations::CallableDeclaration::body),
            Some(entry.body())
        );
    }
}

#[test]
fn executable_entry_rejects_missing_non_function_and_invalid_callable_contracts() {
    let cases = [
        ("", None),
        ("struct main {}\n", None),
        (
            "func main<T>(): void { return }\n",
            Some(crate::EntryContractRule::GenericParameters),
        ),
        (
            "func main(value: i32): void { return }\n",
            Some(crate::EntryContractRule::ValueParameters),
        ),
        (
            "func main(): u64 { return 0 }\n",
            Some(crate::EntryContractRule::ResultType),
        ),
    ];
    for (source, expected_rule) in cases {
        let target = build_target_program(&Fixture::with_app(source));
        let (target_id, _) = target
            .checked()
            .graph()
            .package_targets()
            .iter()
            .next()
            .unwrap();
        let error = select_executable_entry(&target, target_id).unwrap_err();
        match (error, expected_rule) {
            (
                EntrySelectionError::MissingMain { .. }
                | EntrySelectionError::InvalidMainEntity { .. },
                None,
            ) => {}
            (EntrySelectionError::InvalidMainContract { rule: actual, .. }, Some(expected)) => {
                assert_eq!(actual, expected);
            }
            _ => panic!("unexpected entry-selection result for {source:?}"),
        }
    }
}

#[test]
fn callable_instance_key_requires_the_complete_concrete_generic_domain() {
    let target = build_target_program(&Fixture::with_app(
        "func helper<T>(value: T): T { return move value }\n\
         func main(): void { return }\n",
    ));
    let graph = target.checked().graph();
    let helper = graph
        .declarations()
        .callables()
        .iter()
        .find_map(|(id, declaration)| {
            (declaration
                .name()
                .and_then(|name| graph.symbols().spelling(name))
                == Some("helper"))
            .then_some(id)
        })
        .unwrap();
    let parameter = graph
        .declarations()
        .callables()
        .get(helper)
        .unwrap()
        .generic_parameters()[0];
    let concrete = target.checked().types().builtin(BuiltinType::I32);
    let key = CallableInstanceKey::new(
        &target,
        helper,
        GenericArguments::new([GenericArgument::new(parameter, concrete)]).unwrap(),
    )
    .unwrap();
    assert_eq!(key.callable(), helper);
    assert_eq!(key.generic_arguments().get(parameter), Some(concrete));

    assert!(matches!(
        CallableInstanceKey::new(&target, helper, GenericArguments::default()),
        Err(CallableInstanceKeyError::GenericDomainMismatch { .. })
    ));
    let symbolic = target
        .checked()
        .types()
        .iter()
        .find_map(|(id, kind)| {
            matches!(kind, TypeKind::GenericParameter(actual) if *actual == parameter).then_some(id)
        })
        .unwrap();
    assert!(matches!(
        CallableInstanceKey::new(
            &target,
            helper,
            GenericArguments::new([GenericArgument::new(parameter, symbolic)]).unwrap(),
        ),
        Err(CallableInstanceKeyError::SymbolicArgument { .. })
    ));
}

#[test]
fn test_target_selects_only_direct_cases_in_source_order() {
    let target = build_target_program(&Fixture::with_tests(
        "test first { return }\n\
         test second { return }\n",
    ));
    let (target_id, _) = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap();
    let selected = select_test_target(&target, target_id).unwrap();
    assert_eq!(selected.target(), target_id);
    assert_eq!(selected.tests().len(), 2);
    assert_eq!(
        selected
            .tests()
            .iter()
            .map(|test| target
                .checked()
                .graph()
                .symbols()
                .spelling(test.name())
                .unwrap())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    for test in selected.tests() {
        assert_eq!(
            target
                .checked()
                .graph()
                .declarations()
                .tests()
                .get(test.declaration())
                .copied()
                .map(nocter_declarations::TestDeclaration::body),
            Some(test.body())
        );
    }
}

#[test]
fn semantic_attachment_cannot_swap_same_shaped_primitive_names() {
    let fixture = Fixture::new();
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let mut bindings = registry.bindings().to_vec();
    let left = bindings
        .iter()
        .position(|binding| binding.role() == PrimitiveRole::CurrentAllocatorState)
        .unwrap();
    let right = bindings
        .iter()
        .position(|binding| binding.role() == PrimitiveRole::CurrentAllocatorKind)
        .unwrap();
    let left_callable = bindings[left].callable();
    let right_callable = bindings[right].callable();
    bindings[left] = PrimitiveBinding::new(bindings[left].role(), right_callable);
    bindings[right] = PrimitiveBinding::new(bindings[right].role(), left_callable);
    let snapshot = ToolchainSnapshot::select(
        CompilationTarget::Arm64Darwin,
        standard_package,
        PrimitiveRegistry::new(bindings).unwrap(),
    )
    .unwrap();
    let error = TargetProgram::build(checked, snapshot).unwrap_err();
    let TargetProgramError::PrimitiveRegistry(PrimitiveRegistryValidationError::Contract(error)) =
        error
    else {
        panic!("unexpected target-program error")
    };
    assert_eq!(error.role(), PrimitiveRole::CurrentAllocatorState);
    assert_eq!(error.rule(), PrimitiveContractRule::Name);
}

fn build_target_program(fixture: &Fixture) -> TargetProgram {
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (program, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, program, source_index).unwrap();
    let output = check_prepared_program(&input, prepared).unwrap();
    let (checked, _) = output.into_parts();
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = registry_for(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
            .unwrap();
    TargetProgram::build(checked, snapshot).unwrap()
}

fn registry_for(checked: &nocter_checking::CheckedProgram) -> PrimitiveRegistry {
    let graph = checked.graph();
    PrimitiveRegistry::new(PrimitiveRole::ALL.iter().copied().map(|role| {
        let callable = graph
            .declarations()
            .callables()
            .iter()
            .find_map(|(callable, declaration)| {
                let CallableOwner::Module(module) = declaration.owner() else {
                    return None;
                };
                let actual_path = graph
                    .modules()
                    .get(module)?
                    .path()
                    .segments()
                    .iter()
                    .map(|segment| graph.symbols().spelling(*segment))
                    .collect::<Option<Vec<_>>>()?;
                (declaration.kind() == CallableKind::Primitive
                    && actual_path == role.module_path()
                    && declaration
                        .name()
                        .and_then(|name| graph.symbols().spelling(name))
                        == Some(role.declaration_name()))
                .then_some(callable)
            })
            .unwrap_or_else(|| panic!("missing fixture primitive {role:?}"));
        PrimitiveBinding::new(role, callable)
    }))
    .unwrap()
}

fn add_parsed(sources: &mut SourceMap, name: &str, text: &str, goal: ParseGoal) -> SyntaxTree {
    let source = sources
        .add_bytes(SourceName::new(name), text.as_bytes())
        .unwrap();
    parsed(sources, source, goal)
}

fn parsed(sources: &SourceMap, source: SourceId, goal: ParseGoal) -> SyntaxTree {
    let tree = parse(sources.get(source).unwrap(), goal);
    assert!(!tree.has_errors(), "{:#?}", tree.diagnostics());
    tree
}

fn package<'syntax>(
    identity: PackageIdentity,
    name: &str,
    path: &str,
    manifest: &'syntax SyntaxTree,
) -> PackageInput<'syntax> {
    PackageInput::new(
        identity,
        name,
        PackageMode::Declared,
        Some(PackageDeclarationInput::new(path, manifest)),
    )
}

fn module<'syntax>(
    package: PackageIdentity,
    path: &[&str],
    source_path: &str,
    source: &'syntax SyntaxTree,
) -> ModuleInput<'syntax> {
    ModuleInput::new(
        ModuleIdentity::new(package, path.iter().copied()),
        vec![ModuleSourceInput::new(
            source_path,
            ModuleSourceKind::Root,
            source,
        )],
    )
}
