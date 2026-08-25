use nocter_compile_input::{ModuleIdentity, StructuralAttachmentInput};
use nocter_declarations::{StandardDeclarationRole, StructuralAttachment};
use nocter_discovery::{
    BuiltinTypeLocator, PrimitiveRoleLocator, StandardRoleLocator, ToolchainRequest,
};
use nocter_model::{BuiltinType, PackageIdentity};
use nocter_runtime_contract::PrimitiveRole;
use nocter_syntax::NodeKind;

/// Builds the exact standard-source profile bundled with this compiler.
///
/// The returned locators are discovery inputs, not semantic identities. Discovery must resolve
/// each locator to one source token before declaration lowering begins.
#[must_use]
pub fn bundled_standard_toolchain(package: &PackageIdentity) -> ToolchainRequest {
    ToolchainRequest::new(
        package.clone(),
        module(package, &["prelude"]),
        structural_attachments(package),
        standard_roles(package),
    )
    .with_primitive_roles(primitive_roles(package))
    .with_builtin_types(builtin_types(package))
}

fn builtin_types(package: &PackageIdentity) -> Vec<BuiltinTypeLocator> {
    BuiltinType::ALL
        .iter()
        .copied()
        .map(|builtin| {
            let path = match builtin {
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
                | BuiltinType::Isize => "num",
                BuiltinType::Str => "str",
                BuiltinType::Error => "error",
                BuiltinType::Void | BuiltinType::Never => "core",
            };
            BuiltinTypeLocator::new(builtin, module(package, &[path]), builtin.spelling())
        })
        .collect()
}

fn structural_attachments(package: &PackageIdentity) -> Vec<StructuralAttachmentInput> {
    vec![StructuralAttachmentInput::new(
        StructuralAttachment::Slice,
        module(package, &["slice"]),
    )]
}

fn standard_roles(package: &PackageIdentity) -> Vec<StandardRoleLocator> {
    use StandardDeclarationRole as Role;

    [
        (
            Role::AbortingAllocator,
            &["mem"][..],
            NodeKind::StructDeclaration,
            "Allocator",
        ),
        (
            Role::AllocationContext,
            &["mem"][..],
            NodeKind::StructDeclaration,
            "AllocationContext",
        ),
        (
            Role::OwnedString,
            &["string"][..],
            NodeKind::StructDeclaration,
            "String",
        ),
        (
            Role::InterpolationConstructor,
            &["string"][..],
            NodeKind::ConstructionFunction,
            "empty",
        ),
        (
            Role::InterpolationTextAppender,
            &["string"][..],
            NodeKind::InherentMethod,
            "push_str",
        ),
        (
            Role::FormatInterface,
            &["fmt"][..],
            NodeKind::InterfaceDeclaration,
            "Format",
        ),
        (
            Role::FormatMethod,
            &["fmt"][..],
            NodeKind::InterfaceMethod,
            "format_into",
        ),
        (
            Role::IteratorInterface,
            &["iter"][..],
            NodeKind::InterfaceDeclaration,
            "Iterator",
        ),
        (
            Role::IteratorItem,
            &["iter"][..],
            NodeKind::AssociatedTypeDeclaration,
            "Item",
        ),
        (
            Role::IteratorNextMethod,
            &["iter"][..],
            NodeKind::InterfaceMethod,
            "next",
        ),
        (
            Role::ExactSizeIteratorInterface,
            &["iter"][..],
            NodeKind::InterfaceDeclaration,
            "ExactSizeIterator",
        ),
        (
            Role::ExactSizeIteratorRemainingLenMethod,
            &["iter"][..],
            NodeKind::InterfaceMethod,
            "remaining_len",
        ),
        (
            Role::ProcessAbort,
            &["process"][..],
            NodeKind::FunctionDeclaration,
            "abort",
        ),
    ]
    .into_iter()
    .map(|(role, path, kind, name)| {
        StandardRoleLocator::new(role, module(package, path), kind, name)
    })
    .collect()
}

fn primitive_roles(package: &PackageIdentity) -> Vec<PrimitiveRoleLocator> {
    PrimitiveRole::ALL
        .iter()
        .copied()
        .map(|role| {
            let (path, name) = primitive_source_location(role);
            PrimitiveRoleLocator::new(role, module(package, path), name)
        })
        .collect()
}

/// The physical source layout of the standard package bundled by this session implementation.
/// Semantic and backend phases receive only the resolved role identities.
fn primitive_source_location(role: PrimitiveRole) -> (&'static [&'static str], &'static str) {
    use PrimitiveRole as Role;

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

fn module(package: &PackageIdentity, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(package.clone(), path.iter().copied())
}
