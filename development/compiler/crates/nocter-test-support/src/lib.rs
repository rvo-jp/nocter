//! Shared source fixtures and public-example contracts for compiler tests.

mod public_examples;

pub use public_examples::{
    PUBLIC_PACKAGE_EXAMPLES, PublicExampleArgument, PublicExampleInput, PublicExampleRun,
    PublicPackageExample,
};

use nocter_compile_input::{
    BuiltinTypeInput, CompileUnitInput, ModuleIdentity, ModuleInput, ModuleSourceInput,
    ModuleSourceKind, PackageInput, PackageMode, PackageTargetResolutionInput, StandardRoleInput,
    StructuralAttachmentInput, ToolchainInput, UseResolutionInput,
};
use nocter_declarations::{StandardDeclarationRole, StructuralAttachment};
use nocter_model::{BuiltinType, CompilationTarget, PackageIdentity};
use nocter_source::{SourceId, SourceMap, SourceName};
use nocter_syntax::{NodeKind, ParseGoal, SyntaxElement, SyntaxTree, parse};

const ERROR_SOURCE: &str = "\
pub primitive type error

primitive func new_error(code: &str, message: &str): error

primitive func context_error(value: error, message: &str): error

primitive func error_code(value: &error): &str from value

primitive func error_message(value: &error): &str from value

construct error {
    pub func new(code: &str, message: &str): Self {
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
        Role::AllocationAbort => (&["internal", "mem"], "allocation_abort"),
        Role::PointerAddress => (&["ptr"], "addr"),
        Role::PointerFromReference => (&["ptr"], "from_ref"),
        Role::PointerFromReadWriteReference => (&["ptr"], "from_ref_mut"),
        Role::PointerFromAddress => (&["internal", "ptr"], "from_addr"),
        Role::PointeeSize => (&["internal", "ptr"], "pointee_size"),
        Role::PointeeAlignment => (&["internal", "ptr"], "pointee_align"),
        Role::CopyStringToPointer => (&["internal", "ptr"], "copy_str_to_ptr"),
        Role::CopyPointerToPointer => (&["internal", "ptr"], "copy_ptr_to_ptr"),
        Role::StoreByteToPointer => (&["internal", "ptr"], "store_u8_to_ptr"),
        Role::StoreValueToPointer => (&["internal", "ptr"], "store_value_to_ptr"),
        Role::DropValueAtPointer => (&["internal", "ptr"], "drop_value_at_ptr"),
        Role::TakeValueAtPointer => (&["internal", "ptr"], "take_value_at_ptr"),
        Role::StringFromRawParts => (&["internal", "ptr"], "str_from_raw_parts"),
        Role::ByteSliceFromRawParts => (&["internal", "ptr"], "slice_from_raw_parts"),
        Role::MutableByteSliceFromRawParts => (&["internal", "ptr"], "slice_from_raw_parts_mut"),
        Role::ValueSliceFromRawParts => (&["internal", "ptr"], "slice_from_raw_parts_value"),
        Role::MutableValueSliceFromRawParts => {
            (&["internal", "ptr"], "slice_from_raw_parts_value_mut")
        }
        Role::BytesFromString => (&["str"], "bytes_from_str"),
        Role::StringSubviewUnchecked => (&["str"], "str_subview_unchecked"),
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
        Role::Syscall0 => (&["internal", "os", "darwin"], "syscall0"),
        Role::Syscall1 => (&["internal", "os", "darwin"], "syscall1"),
        Role::Syscall2 => (&["internal", "os", "darwin"], "syscall2"),
        Role::Syscall3 => (&["internal", "os", "darwin"], "syscall3"),
        Role::Syscall4 => (&["internal", "os", "darwin"], "syscall4"),
        Role::Syscall5 => (&["internal", "os", "darwin"], "syscall5"),
        Role::Syscall6 => (&["internal", "os", "darwin"], "syscall6"),
        Role::Trap => (&["internal", "os", "darwin"], "trap"),
        Role::Unreachable => (&["internal", "os", "darwin"], "unreachable"),
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
primitive func current_allocator_state(): usize
primitive func current_allocator_kind(): usize
primitive func allocation_failure_error(): error
pub func allocation_context_state_for_test(): usize {
    return current_allocator_state()
}
pub func allocation_context_kind_for_test(): usize {
    return current_allocator_kind()
}
pub func allocation_failure_for_test(): error { return allocation_failure_error() }
";
const INTERNAL_MEM_SOURCE: &str = "\
pub(/) primitive func allocation_abort(): never
pub func allocation_abort_for_test(): never { allocation_abort() }
";
const PTR_SOURCE: &str = "\
pub primitive func addr<T>(pointer: *T): usize
pub primitive func from_ref<T>(value: &T): *T
pub primitive func from_ref_mut<T>(value: &+T): *T
";
const INTERNAL_PTR_SOURCE: &str = "\
pub(/) primitive func from_addr<T>(address: usize): *T
pub(/) primitive func pointee_size<T>(pointer: *T): usize
pub(/) primitive func pointee_align<T>(pointer: *T): usize
pub(/) primitive func copy_str_to_ptr(destination: *u8, offset: usize, text: &str): void
pub(/) primitive func copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
pub(/) primitive func store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
pub(/) primitive func store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
pub(/) primitive func drop_value_at_ptr<T>(pointer: *T, offset: usize): void
pub(/) primitive func take_value_at_ptr<T>(pointer: *T, offset: usize): T
pub(/) primitive func str_from_raw_parts(pointer: *u8, len: usize): &str
pub(/) primitive func slice_from_raw_parts(pointer: *u8, len: usize): &[u8]
pub(/) primitive func slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
pub(/) primitive func slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
pub(/) primitive func slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
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
const STRING_SOURCE: &str = "";
const SLICE_SOURCE: &str = "\
primitive func slice_len_raw<T>(value: &[T]): usize
primitive func slice_ptr_addr_raw<T>(value: &[T]): usize
pub func slice_len_for_test<T>(value: &[T]): usize { return slice_len_raw(value) }
pub func slice_ptr_addr_for_test<T>(value: &[T]): usize { return slice_ptr_addr_raw(value) }
";
const STR_SOURCE: &str = "\
pub primitive type str

primitive func bytes_from_str(value: &str): &[u8] from value
primitive func str_subview_unchecked(text: &str, start: usize, len: usize): &str from text
primitive func str_len_raw(value: &str): usize
primitive func str_ptr_addr_raw(value: &str): usize
pub func bytes_from_str_for_test(value: &str): &[u8] { return bytes_from_str(value) }
pub func str_subview_unchecked_for_test(text: &str, start: usize, len: usize): &str {
    return str_subview_unchecked(text, start, len)
}
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
const NUM_SOURCE: &str = "\
pub primitive type bool
pub primitive type i8
pub primitive type i16
pub primitive type i32
pub primitive type i64
pub primitive type u8
pub primitive type u16
pub primitive type u32
pub primitive type u64
pub primitive type usize
pub primitive type isize
";
const CORE_SOURCE: &str = "\
pub primitive type void
pub primitive type never
";
const PROCESS_SOURCE: &str = "\
#target: \"arm64-darwin\"
primitive func exit_raw(code: i32): never
#target: \"arm64-darwin\"
primitive func arg_count_raw(): usize
#target: \"arm64-darwin\"
primitive func arg_raw(index: usize): &str from static
#target: \"arm64-darwin\"
primitive func env_count_raw(): usize
#target: \"arm64-darwin\"
primitive func env_name_raw(index: usize): &str from static
#target: \"arm64-darwin\"
primitive func env_value_raw(index: usize): &str from static
pub func exit_for_test(code: i32): never { exit_raw(code) }
pub func arg_count_for_test(): usize { return arg_count_raw() }
pub func arg_for_test(index: usize): &str { return arg_raw(index) }
pub func env_count_for_test(): usize { return env_count_raw() }
pub func env_name_for_test(index: usize): &str { return env_name_raw(index) }
pub func env_value_for_test(index: usize): &str { return env_value_raw(index) }
";
const IO_SOURCE: &str = "pub func answer_for_test(): i32 { return 42 }\n";
const INTERNAL_OS_SOURCE: &str = "\
#target: \"arm64-darwin\"
pub(/) copy struct SyscallResult {
    pub value: usize
    pub errno: i32
}
#target: \"arm64-darwin\"
primitive func syscall0(number: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive func syscall1(number: usize, a0: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive func syscall2(number: usize, a0: usize, a1: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive func syscall3(number: usize, a0: usize, a1: usize, a2: usize): SyscallResult
#target: \"arm64-darwin\"
primitive func syscall4(number: usize, a0: usize, a1: usize, a2: usize, a3: usize): SyscallResult
#target: \"arm64-darwin\"
primitive func syscall5(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive func syscall6(number: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize): SyscallResult
#target: \"arm64-darwin\"
pub(/) primitive func trap(): never
#target: \"arm64-darwin\"
primitive func unreachable(): never
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
    app_is_package: bool,
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
        app_package_directives: Option<&str>,
        standard_source: &str,
        standard_roles: impl Into<Box<[StandardRoleSpec]>>,
    ) -> Self {
        let mut sources = SourceMap::new();
        let app_is_package = app_package_directives.is_some();
        let app_path = if app_is_package {
            "/app/index.nct"
        } else {
            "/tmp/app.nct"
        };
        let app_source = app_package_directives.map_or_else(
            || app_source.to_owned(),
            |directives| {
                format!(
                    "#package: {{ name: \"app\", version: \"0.0.0\", }}\n{directives}{app_source}"
                )
            },
        );
        let app = add_parsed(&mut sources, app_path, &app_source, ParseGoal::SourceFile);
        let standard_source =
            format!("#package: {{ name: \"std\", version: \"0.0.0\", }}\n{standard_source}");
        let standard = add_parsed(
            &mut sources,
            "/std/index.nct",
            &standard_source,
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
            (&["core"][..], CORE_SOURCE),
            (&["num"][..], NUM_SOURCE),
            (&["mem"][..], MEM_SOURCE),
            (&["internal", "mem"][..], INTERNAL_MEM_SOURCE),
            (&["ptr"][..], PTR_SOURCE),
            (&["internal", "ptr"][..], INTERNAL_PTR_SOURCE),
            (&["string"][..], STRING_SOURCE),
            (&["slice"][..], SLICE_SOURCE),
            (&["str"][..], STR_SOURCE),
            (&["process"][..], PROCESS_SOURCE),
            (&["io"][..], IO_SOURCE),
            (&["internal", "os", "darwin"][..], INTERNAL_OS_SOURCE),
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
            app_is_package,
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
    /// Panics when an internally authored package source lacks its required target directive.
    #[must_use]
    pub fn input(&self) -> CompileUnitInput<'_> {
        let app_package = PackageIdentity::new("workspace:app");
        let standard_package = PackageIdentity::new("toolchain:std");
        let packages = vec![
            PackageInput::new(
                app_package.clone(),
                "app",
                if self.app_is_package {
                    PackageMode::Declared
                } else {
                    PackageMode::SingleFile
                },
            ),
            package(
                standard_package.clone(),
                "std",
                "/std/index.nct",
                &self.standard,
            ),
        ];
        let mut modules = vec![
            ModuleInput::new(
                ModuleIdentity::new(app_package.clone(), Vec::<&str>::new()),
                vec![ModuleSourceInput::new(
                    if self.app_is_package {
                        "/app/index.nct"
                    } else {
                        "/tmp/app.nct"
                    },
                    if self.app_is_package {
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
        if self.app_is_package {
            let source = self.sources.get(self.app.source()).unwrap();
            let declaration = nocter_package::decode_package_declaration(source, &self.app)
                .expect("test fixture package source is valid");
            let target = declaration
                .targets()
                .first()
                .expect("test fixture package source has no target directive");
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
        ToolchainInput::new(
            standard.clone(),
            ModuleIdentity::new(standard.clone(), ["prelude"]),
            vec![StructuralAttachmentInput::new(
                StructuralAttachment::Slice,
                ModuleIdentity::new(standard.clone(), ["slice"]),
            )],
            self.standard_role_inputs(),
        )
        .with_builtin_types(self.builtin_type_inputs())
    }

    fn builtin_type_inputs(&self) -> Vec<BuiltinTypeInput> {
        BuiltinType::ALL
            .iter()
            .copied()
            .map(|builtin| {
                BuiltinTypeInput::new(
                    builtin,
                    declaration_token(
                        &self.sources,
                        self.module_for_builtin(builtin),
                        NodeKind::PrimitiveTypeDeclaration,
                        builtin.spelling(),
                    ),
                )
            })
            .collect()
    }

    fn module_for_builtin(&self, builtin: BuiltinType) -> &SyntaxTree {
        let path: &[&str] = match builtin {
            BuiltinType::Bool
            | BuiltinType::I8
            | BuiltinType::I16
            | BuiltinType::I32
            | BuiltinType::I64
            | BuiltinType::U8
            | BuiltinType::U16
            | BuiltinType::U32
            | BuiltinType::U64
            | BuiltinType::Usize
            | BuiltinType::Isize => &["num"],
            BuiltinType::Str => &["str"],
            BuiltinType::Error => &["error"],
            BuiltinType::Void | BuiltinType::Never => &["core"],
        };
        &self
            .modules
            .iter()
            .find(|module| {
                module
                    .path
                    .iter()
                    .map(AsRef::as_ref)
                    .eq(path.iter().copied())
            })
            .expect("built-in fixture module exists")
            .syntax
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

fn package(
    identity: PackageIdentity,
    name: &str,
    _path: &str,
    _manifest: &SyntaxTree,
) -> PackageInput {
    PackageInput::new(identity, name, PackageMode::Declared)
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
