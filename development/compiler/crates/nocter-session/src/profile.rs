use nocter_compile_input::{BuiltinAttachmentInput, ModuleIdentity, PackageIdentity};
use nocter_declarations::{BuiltinAttachment, PrimitiveRole, StandardDeclarationRole};
use nocter_discovery::{PrimitiveRoleLocator, StandardRoleLocator, ToolchainRequest};
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
            let (path, name) = nocter_target_program::primitive_source_location(role);
            PrimitiveRoleLocator::new(
                role,
                module(package, path),
                NodeKind::PrimitiveDeclaration,
                name,
            )
        })
        .collect()
}

fn module(package: &PackageIdentity, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(package.clone(), path.iter().copied())
}
