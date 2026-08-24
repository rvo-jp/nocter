//! Shared source fixtures and public-example contracts for compiler tests.

mod public_examples;

pub use public_examples::{
    PUBLIC_PACKAGE_EXAMPLES, PublicExampleArgument, PublicExampleInput, PublicExampleRun,
    PublicPackageExample,
};

use nocter_compile_input::{
    BuiltinAttachmentInput, CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageDeclarationInput, PackageInput, PackageMode,
    PackageTargetResolutionInput, StandardRoleInput, ToolchainInput, UseResolutionInput,
};
use nocter_declarations::{BuiltinAttachment, StandardDeclarationRole};
use nocter_model::{CompilationTarget, PackageIdentity};
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

const ERROR_SOURCE: &str = "\
primitive new_error(code: &str, message: &str): error

primitive context_error(value: error, message: &str): error

primitive error_code(value: &error): &str from value

primitive error_message(value: &error): &str from value

construct error {
    pub default func new(code: &str, message: &str): Self {
        return new_error(code, message)
    }
}
instance error {
    pub method self.context(message: &str): Self {
        return context_error(move self, message)
    }

    pub method &self.code(): &str from self {
        return error_code(self)
    }

    pub method &self.message(): &str from self {
        return error_message(self)
    }

    pub method &self.has_code(code: &str): bool {
        return error_code(self) == code
    }

}
";

/// Physical primitive locators belonging to the shared authored standard fixture.
#[must_use]
pub fn primitive_source_location(
    role: nocter_runtime_contract::PrimitiveRole,
) -> (&'static [&'static str], &'static str) {
    use nocter_runtime_contract::PrimitiveRole as Role;

    match role {
        Role::NewError => (&["error"], "new_error"),
        Role::ErrorContext => (&["error"], "context_error"),
        Role::ErrorCode => (&["error"], "error_code"),
        Role::ErrorMessage => (&["error"], "error_message"),
        Role::AllocationFailureError => (&["mem"], "allocation_failure_error"),
        Role::CurrentAllocatorState => (&["mem"], "current_allocator_state"),
        Role::CurrentAllocatorKind => (&["mem"], "current_allocator_kind"),
        Role::AllocationAbort => (&["mem"], "allocation_abort_raw"),
        Role::PointerAddress => (&["ptr"], "addr"),
        Role::PointerFromReference => (&["ptr"], "from_ref"),
        Role::PointerFromReadWriteReference => (&["ptr"], "from_ref_mut"),
        Role::PointerFromAddress => (&["ptr"], "from_addr"),
        Role::PointeeSize => (&["ptr"], "pointee_size"),
        Role::PointeeAlignment => (&["ptr"], "pointee_align"),
        Role::CopyStringToPointer => (&["ptr"], "copy_str_to_ptr"),
        Role::CopyPointerToPointer => (&["ptr"], "copy_ptr_to_ptr"),
        Role::StoreByteToPointer => (&["ptr"], "store_u8_to_ptr"),
        Role::StoreValueToPointer => (&["ptr"], "store_value_to_ptr"),
        Role::DropValueAtPointer => (&["ptr"], "drop_value_at_ptr"),
        Role::TakeValueAtPointer => (&["ptr"], "take_value_at_ptr"),
        Role::StringFromRawParts => (&["ptr"], "str_from_raw_parts"),
        Role::ByteSliceFromRawParts => (&["ptr"], "slice_from_raw_parts"),
        Role::MutableByteSliceFromRawParts => (&["ptr"], "slice_from_raw_parts_mut"),
        Role::ValueSliceFromRawParts => (&["ptr"], "slice_from_raw_parts_value"),
        Role::MutableValueSliceFromRawParts => (&["ptr"], "slice_from_raw_parts_value_mut"),
        Role::BytesFromString => (&["string"], "bytes_from_str"),
        Role::StringSubviewUnchecked => (&["string"], "str_subview_unchecked"),
        Role::SliceLength => (&["slice"], "slice_len_raw"),
        Role::SlicePointerAddress => (&["slice"], "slice_ptr_addr_raw"),
        Role::StringLength => (&["str"], "str_len_raw"),
        Role::StringPointerAddress => (&["str"], "str_ptr_addr_raw"),
        Role::ProcessExit => (&["process"], "exit_raw"),
        Role::ProcessArgumentCount => (&["process"], "arg_count_raw"),
        Role::ProcessArgument => (&["process"], "arg_raw"),
        Role::ProcessEnvironmentCount => (&["process"], "env_count_raw"),
        Role::ProcessEnvironmentName => (&["process"], "env_name_raw"),
        Role::ProcessEnvironmentValue => (&["process"], "env_value_raw"),
        Role::Syscall0 => (&["internal", "os"], "syscall0"),
        Role::Syscall1 => (&["internal", "os"], "syscall1"),
        Role::Syscall2 => (&["internal", "os"], "syscall2"),
        Role::Syscall3 => (&["internal", "os"], "syscall3"),
        Role::Syscall4 => (&["internal", "os"], "syscall4"),
        Role::Syscall5 => (&["internal", "os"], "syscall5"),
        Role::Syscall6 => (&["internal", "os"], "syscall6"),
        Role::Trap => (&["internal", "os"], "trap"),
        Role::Unreachable => (&["internal", "os"], "unreachable"),
    }
}
const ITERATION_SOURCE: &str = "\
pub interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}
pub interface ExactSizeIterator {
    pub method &self.remaining_len(): usize
}
";
const INTERPOLATION_SOURCE: &str = "\
pub struct String {}
construct String {
    pub func empty(): Self { return Self {} }
}
instance String {
    pub method &+self.push_str(text: &str): void { return }
}
drop String(&+self) { return }
pub interface Format {
    pub method &self.format_into(output: &+String): void
}
";
const ALLOCATION_SOURCE: &str = "\
pub struct Allocator { pub state: usize
    pub kind: usize }
pub struct AllocationContext { state: usize
    kind: usize }
";
const MEM_SOURCE: &str = "\
primitive current_allocator_state(): usize
primitive current_allocator_kind(): usize
primitive allocation_failure_error(): error
pub(/) primitive allocation_abort_raw(): never
pub func allocation_context_state_for_test(): usize {
    return current_allocator_state()
}
pub func allocation_context_kind_for_test(): usize {
    return current_allocator_kind()
}
pub func allocation_failure_for_test(): error { return allocation_failure_error() }
pub func allocation_abort_for_test(): never { allocation_abort_raw() }
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
pub func pointee_size_for_test<T>(pointer: *T): usize { return pointee_size(pointer) }
pub func pointee_align_for_test<T>(pointer: *T): usize { return pointee_align(pointer) }
pub func copy_str_to_ptr_for_test(destination: *u8, offset: usize, text: &str): void {
    copy_str_to_ptr(destination, offset, text)
    return
}
pub func copy_ptr_to_ptr_for_test(destination: *u8, source: *u8, byte_count: usize): void {
    copy_ptr_to_ptr(destination, source, byte_count)
    return
}
pub func store_u8_to_ptr_for_test(
    destination: *u8,
    offset: usize,
    value: u8,
): void {
    store_u8_to_ptr(destination, offset, value)
    return
}
pub func store_value_to_ptr_for_test<T>(destination: *T, offset: usize, value: T): void {
    store_value_to_ptr(destination, offset, move value)
    return
}
pub func drop_value_at_ptr_for_test<T>(pointer: *T, offset: usize): void {
    drop_value_at_ptr(pointer, offset)
    return
}
pub func take_u64_at_ptr_for_test(pointer: *u64, offset: usize): u64 {
    return take_value_at_ptr(pointer, offset)
}
pub func take_three_u64_at_ptr_for_test(pointer: *[u64; 3], offset: usize): [u64; 3] {
    return take_value_at_ptr(pointer, offset)
}
";
const STRING_SOURCE: &str = "\
pub(/) primitive bytes_from_str(value: &str): &[u8] from value
primitive str_subview_unchecked(text: &str, start: usize, len: usize): &str from text
pub func bytes_from_str_for_test(value: &str): &[u8] { return bytes_from_str(value) }
pub func str_subview_unchecked_for_test(text: &str, start: usize, len: usize): &str {
    return str_subview_unchecked(text, start, len)
}
";
const SLICE_SOURCE: &str = "\
primitive slice_len_raw<T>(value: &[T]): usize
primitive slice_ptr_addr_raw<T>(value: &[T]): usize
pub func slice_len_for_test<T>(value: &[T]): usize { return slice_len_raw(value) }
pub func slice_ptr_addr_for_test<T>(value: &[T]): usize { return slice_ptr_addr_raw(value) }
";
const STR_SOURCE: &str = "\
primitive str_len_raw(value: &str): usize
primitive str_ptr_addr_raw(value: &str): usize
instance str {
    pub operator (&self == other: &Self): bool {
        if str_len_raw(self) != str_len_raw(other) { return false }
        var index: usize = 0
        while index < str_len_raw(self) {
            if self[index] != other[index] { return false }
            index += 1
        }
        return true
    }
}
pub func str_len_for_test(value: &str): usize { return str_len_raw(value) }
pub func str_ptr_addr_for_test(value: &str): usize { return str_ptr_addr_raw(value) }
";
const PROCESS_SOURCE: &str = "\
#target: \"arm64-darwin\"
primitive exit_raw(code: i32): never
#target: \"arm64-darwin\"
primitive arg_count_raw(): usize
#target: \"arm64-darwin\"
primitive arg_raw(index: usize): &str from static
#target: \"arm64-darwin\"
primitive env_count_raw(): usize
#target: \"arm64-darwin\"
primitive env_name_raw(index: usize): &str from static
#target: \"arm64-darwin\"
primitive env_value_raw(index: usize): &str from static
pub func exit_for_test(code: i32): never { exit_raw(code) }
pub func arg_count_for_test(): usize { return arg_count_raw() }
pub func arg_for_test(index: usize): &str { return arg_raw(index) }
pub func env_count_for_test(): usize { return env_count_raw() }
pub func env_name_for_test(index: usize): &str { return env_name_raw(index) }
pub func env_value_for_test(index: usize): &str { return env_value_raw(index) }
";
const IO_SOURCE: &str = "";
const INTERNAL_OS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}
#target: \"arm64-darwin\"
primitive syscall0(number: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall1(number: usize, a0: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall2(number: usize, a0: usize, a1: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
#target: \"arm64-darwin\"
primitive syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
#target: \"arm64-darwin\"
primitive syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive trap(): never
#target: \"arm64-darwin\"
primitive unreachable(): never
pub func syscall0_succeeds_for_test(number: usize): bool {
    let result = syscall0(number)
    return result.errno == 0 && result.value > 0
}
pub func syscall1_fails_for_test(number: usize, argument: usize): bool {
    let result = syscall1(number, argument)
    return result.errno != 0
}
pub func syscall3_value_for_test(
    number: usize,
    a0: usize,
    a1: usize,
    a2: usize,
): usize {
    let result = syscall3(number, a0, a1, a2)
    if result.errno != 0 { return 18446744073709551615 }
    return result.value
}
pub func terminate_for_test(use_unreachable: bool): never {
    if use_unreachable { unreachable() }
    trap()
}
";

struct FixtureModule {
    path: Box<[Box<str>]>,
    source_path: Box<str>,
    syntax: SyntaxTree,
}

#[derive(Clone, Copy)]
struct StandardRoleSpec {
    role: StandardDeclarationRole,
    kind: NodeKind,
    name: &'static str,
}

const ITERATION_ROLES: &[StandardRoleSpec] = &[
    StandardRoleSpec {
        role: StandardDeclarationRole::IteratorInterface,
        kind: NodeKind::InterfaceDeclaration,
        name: "Iterator",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::IteratorItem,
        kind: NodeKind::AssociatedTypeDeclaration,
        name: "Item",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::IteratorNextMethod,
        kind: NodeKind::InterfaceMethod,
        name: "next",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::ExactSizeIteratorInterface,
        kind: NodeKind::InterfaceDeclaration,
        name: "ExactSizeIterator",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::ExactSizeIteratorRemainingLenMethod,
        kind: NodeKind::InterfaceMethod,
        name: "remaining_len",
    },
];

const INTERPOLATION_ROLES: &[StandardRoleSpec] = &[
    StandardRoleSpec {
        role: StandardDeclarationRole::OwnedString,
        kind: NodeKind::StructDeclaration,
        name: "String",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::InterpolationConstructor,
        kind: NodeKind::ConstructionFunction,
        name: "empty",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::InterpolationTextAppender,
        kind: NodeKind::InherentMethod,
        name: "push_str",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::FormatInterface,
        kind: NodeKind::InterfaceDeclaration,
        name: "Format",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::FormatMethod,
        kind: NodeKind::InterfaceMethod,
        name: "format_into",
    },
];

const ALLOCATION_ROLES: &[StandardRoleSpec] = &[
    StandardRoleSpec {
        role: StandardDeclarationRole::AbortingAllocator,
        kind: NodeKind::StructDeclaration,
        name: "Allocator",
    },
    StandardRoleSpec {
        role: StandardDeclarationRole::AllocationContext,
        kind: NodeKind::StructDeclaration,
        name: "AllocationContext",
    },
];

/// Complete source input containing an application and the minimum compiler-selected standard
/// surface required to construct an `arm64-darwin` target program.
pub struct CompilerFixture {
    sources: SourceMap,
    app_manifest: Option<SyntaxTree>,
    std_manifest: SyntaxTree,
    app: SyntaxTree,
    standard: SyntaxTree,
    prelude: SyntaxTree,
    modules: Vec<FixtureModule>,
    app_standard_uses: Vec<Box<[Box<str>]>>,
    standard_roles: Box<[StandardRoleSpec]>,
}

impl CompilerFixture {
    #[must_use]
    pub fn new() -> Self {
        Self::with_app("func main(): i32! { return 0 }\n")
    }

    #[must_use]
    pub fn with_app(app_source: &str) -> Self {
        Self::build(app_source, None, "", [])
    }

    /// Builds the complete target fixture with the compiler-selected iterator semantic surface.
    #[must_use]
    pub fn with_app_iteration(app_source: &str) -> Self {
        Self::build(app_source, None, ITERATION_SOURCE, ITERATION_ROLES)
    }

    /// Builds the complete target fixture with compiler-selected interpolation semantics.
    #[must_use]
    pub fn with_app_interpolation(app_source: &str) -> Self {
        Self::build(app_source, None, INTERPOLATION_SOURCE, INTERPOLATION_ROLES)
    }

    /// Builds the interpolation fixture and resolves application `use` declarations in source
    /// order.
    #[must_use]
    pub fn with_app_interpolation_standard_uses(app_source: &str, modules: &[&[&str]]) -> Self {
        Self::with_standard_uses(Self::with_app_interpolation(app_source), modules)
    }

    /// Builds the iterator fixture and resolves application `use` declarations to standard
    /// modules in source order.
    #[must_use]
    pub fn with_app_iteration_standard_uses(app_source: &str, modules: &[&[&str]]) -> Self {
        Self::with_standard_uses(Self::with_app_iteration(app_source), modules)
    }

    /// Builds the complete target fixture with compiler-selected lexical-region semantics.
    #[must_use]
    pub fn with_app_allocation(app_source: &str) -> Self {
        Self::build(app_source, None, ALLOCATION_SOURCE, ALLOCATION_ROLES)
    }

    /// Builds the allocation fixture and resolves application `use` declarations in source order.
    #[must_use]
    pub fn with_app_allocation_standard_uses(app_source: &str, modules: &[&[&str]]) -> Self {
        Self::with_standard_uses(Self::with_app_allocation(app_source), modules)
    }

    #[must_use]
    pub fn with_tests(app_source: &str) -> Self {
        Self::build(
            app_source,
            Some("#test: { name: \"unit\", module: \".\", }\n"),
            "",
            [],
        )
    }

    #[must_use]
    pub fn with_app_standard_uses(app_source: &str, modules: &[&[&str]]) -> Self {
        Self::with_standard_uses(Self::with_app(app_source), modules)
    }

    fn with_standard_uses(mut fixture: Self, modules: &[&[&str]]) -> Self {
        fixture.app_standard_uses = modules
            .iter()
            .map(|segments| {
                segments
                    .iter()
                    .map(|segment| Box::<str>::from(*segment))
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .collect();
        fixture
    }

    fn build(
        app_source: &str,
        app_manifest_source: Option<&str>,
        standard_source: &str,
        standard_roles: impl Into<Box<[StandardRoleSpec]>>,
    ) -> Self {
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
        let app = add_parsed(&mut sources, app_path, app_source, ParseGoal::SourceFile);
        let standard = add_parsed(
            &mut sources,
            "/std/index.nct",
            standard_source,
            ParseGoal::SourceFile,
        );
        let prelude = add_parsed(
            &mut sources,
            "/std/prelude/index.nct",
            "",
            ParseGoal::SourceFile,
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
            let syntax = add_parsed(&mut sources, &source_path, text, ParseGoal::SourceFile);
            FixtureModule {
                path: path
                    .iter()
                    .map(|segment| Box::<str>::from(*segment))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                source_path: source_path.into_boxed_str(),
                syntax,
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
            app_standard_uses: Vec::new(),
            standard_roles: standard_roles.into(),
        }
    }

    /// Returns a borrowing compile-unit input with its exact toolchain profile.
    ///
    /// # Panics
    ///
    /// Panics when an internally authored fixture manifest lacks its required target directive.
    #[must_use]
    pub fn input(&self) -> CompileUnitInput<'_> {
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
        let use_resolutions = self.app_use_resolutions(&standard_package);
        let toolchain = self.toolchain_input(&standard_package);
        let mut input = CompileUnitInput::new(
            CompilationTarget::Arm64Darwin,
            &self.sources,
            packages,
            modules,
            use_resolutions,
        )
        .with_root_packages(vec![app_package.clone()])
        .with_toolchain(toolchain);
        if let Some(manifest) = &self.app_manifest {
            let source = self.sources.get(manifest.source()).unwrap();
            let declaration = nocter_package::decode_package_declaration(source, manifest)
                .expect("test fixture manifest is valid");
            let target = declaration
                .targets()
                .first()
                .expect("test fixture manifest has no target directive");
            input = input.with_package_target_resolutions(vec![PackageTargetResolutionInput::new(
                target.declaration(),
                target.name().value(),
                target.name().literal(),
                target.kind(),
                target.order(),
                ModuleIdentity::new(app_package, Vec::<&str>::new()),
            )]);
        }
        input
    }

    fn toolchain_input(&self, standard: &PackageIdentity) -> ToolchainInput {
        let attachment = |kind, path| {
            BuiltinAttachmentInput::new(kind, ModuleIdentity::new(standard.clone(), [path]))
        };
        ToolchainInput::new(
            standard.clone(),
            ModuleIdentity::new(standard.clone(), ["prelude"]),
            vec![
                attachment(BuiltinAttachment::Str, "str"),
                attachment(BuiltinAttachment::Error, "error"),
                attachment(BuiltinAttachment::Slice, "slice"),
            ],
            self.standard_role_inputs(),
        )
    }

    fn standard_role_inputs(&self) -> Vec<StandardRoleInput> {
        self.standard_roles
            .iter()
            .copied()
            .map(|spec| {
                StandardRoleInput::new(
                    spec.role,
                    declaration_token(&self.sources, &self.standard, spec.kind, spec.name),
                )
            })
            .collect()
    }

    fn app_use_resolutions(&self, standard: &PackageIdentity) -> Vec<UseResolutionInput> {
        let nodes = use_declarations(&self.app);
        assert_eq!(nodes.len(), self.app_standard_uses.len());
        nodes
            .into_iter()
            .zip(&self.app_standard_uses)
            .map(|(declaration, path)| {
                UseResolutionInput::new(
                    declaration,
                    ModuleIdentity::new(standard.clone(), path.iter().map(AsRef::as_ref)),
                )
            })
            .collect()
    }
}

impl Default for CompilerFixture {
    fn default() -> Self {
        Self::new()
    }
}

fn use_declarations(tree: &SyntaxTree) -> Vec<nocter_syntax::NodeId> {
    let mut declarations = Vec::new();
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|node| node.kind() == NodeKind::UseDeclaration)
        {
            declarations.push(node);
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    declarations.sort_unstable_by_key(|node| {
        tree.node(*node)
            .map(nocter_syntax::SyntaxNode::range)
            .map(nocter_source::TextRange::start)
    });
    declarations
}

fn declaration_token(
    sources: &SourceMap,
    tree: &SyntaxTree,
    kind: NodeKind,
    name: &str,
) -> nocter_syntax::SyntaxToken {
    let source = sources.get(tree.source()).expect("fixture source");
    let mut pending = vec![tree.root_id()];
    while let Some(node) = pending.pop() {
        if tree.node(node).is_some_and(|node| node.kind() == kind) {
            let mut descendants = vec![node];
            while let Some(descendant) = descendants.pop() {
                for child in tree.children(descendant).iter().rev() {
                    match child {
                        SyntaxElement::Token(token)
                            if source
                                .text_at(token.range())
                                .is_some_and(|text| text == name) =>
                        {
                            return *token;
                        }
                        SyntaxElement::Node(child) => descendants.push(*child),
                        SyntaxElement::Token(_) | SyntaxElement::Missing(_) => {}
                    }
                }
            }
        }
        for child in tree.children(node).iter().rev() {
            if let SyntaxElement::Node(child) = child {
                pending.push(*child);
            }
        }
    }
    panic!("fixture declaration {kind:?} named {name} is missing")
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
