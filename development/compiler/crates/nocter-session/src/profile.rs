use nocter_compile_input::{BuiltinAttachmentInput, ModuleIdentity};
use nocter_declarations::{BuiltinAttachment, StandardDeclarationRole};
use nocter_discovery::{PrimitiveRoleLocator, StandardRoleLocator, ToolchainRequest};
use nocter_model::PackageIdentity;
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
        builtin_attachments(package),
        standard_roles(package),
    )
    .with_primitive_roles(primitive_roles(package))
}

fn builtin_attachments(package: &PackageIdentity) -> Vec<BuiltinAttachmentInput> {
    [
        (BuiltinAttachment::Scalar, "num"),
        (BuiltinAttachment::Str, "str"),
        (BuiltinAttachment::Error, "error"),
        (BuiltinAttachment::Slice, "slice"),
    ]
    .into_iter()
    .map(|(attachment, path)| BuiltinAttachmentInput::new(attachment, module(package, &[path])))
    .collect()
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
            PrimitiveRoleLocator::new(
                role,
                module(package, path),
                NodeKind::PrimitiveDeclaration,
                name,
            )
        })
        .collect()
}

/// The physical source layout of the standard package bundled by this session implementation.
/// Semantic and backend phases receive only the resolved role identities.
fn primitive_source_location(role: PrimitiveRole) -> (&'static [&'static str], &'static str) {
    use PrimitiveRole as Role;

    match role {
        Role::NewError => (&["error"], "new_error"),
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

fn module(package: &PackageIdentity, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(package.clone(), path.iter().copied())
}
