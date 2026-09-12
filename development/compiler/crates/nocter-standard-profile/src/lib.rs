//! Physical declaration profile for the standard package bundled with this compiler.

use nocter_compile_input::{
    BuiltinTypeLocator, ModuleIdentity, PrimitiveRoleLocator, RuntimeStorageRoleLocator,
    StandardRoleLocator, StructuralAttachmentInput, TargetServiceRoleLocator, ToolchainInput,
};
use nocter_model::{BuiltinType, PackageIdentity};
use nocter_runtime_contract::{PrimitiveRole, RuntimeStorageRole, TargetServiceRole};
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

const DARWIN_NET: &[&str] = &["internal", "net", "darwin"];
const INTERNAL_TASK: &[&str] = &["internal", "task"];
const READINESS_OR_DEADLINE: &str = "descriptor_readiness_or_deadline_raw";
const NET_EVENT_DESCRIPTOR: &str = "network_connection_event_descriptor_raw";
const NET_BEGIN_RECEIVE: &str = "network_connection_begin_receive_raw";
const NET_BEGIN_SEND: &str = "network_connection_begin_send_raw";
const NET_RECEIVE_EVENT: &str = "network_connection_receive_event_raw";
const NET_COPY_LOCAL_ADDRESS: &str = "network_connection_copy_local_address_raw";
const NET_COPY_REMOTE_ADDRESS: &str = "network_connection_copy_remote_address_raw";
const NET_REQUEST_CANCEL: &str = "network_connection_request_cancel_raw";
const NET_RELEASE_BARRIER: &str = "network_connection_release_barrier_raw";

/// Builds the exact standard-source profile bundled with this compiler.
///
/// The returned locators are resolved against target-filtered declaration surfaces by lowering.
#[must_use]
pub fn bundled_standard_toolchain(package: &PackageIdentity) -> ToolchainInput {
    ToolchainInput::new(
        package.clone(),
        module(package, &["prelude"]),
        structural_attachments(package),
        standard_roles(package),
    )
    .with_primitive_roles(primitive_roles(package))
    .with_target_service_roles(target_service_roles(package))
    .with_runtime_storage_roles(runtime_storage_roles(package))
    .with_builtin_types(builtin_types(package))
}

fn runtime_storage_roles(package: &PackageIdentity) -> Vec<RuntimeStorageRoleLocator> {
    [(RuntimeStorageRole::NetworkOwner, "NetworkOwner")]
        .into_iter()
        .map(|(role, name)| {
            RuntimeStorageRoleLocator::new(
                role,
                module(package, &["internal", "net", "model"]),
                name,
            )
        })
        .collect()
}

fn builtin_types(package: &PackageIdentity) -> Vec<BuiltinTypeLocator> {
    BuiltinType::ALL
        .iter()
        .copied()
        .map(|builtin| {
            let path = match builtin {
                BuiltinType::Bool
                | BuiltinType::F32
                | BuiltinType::F64
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
                BuiltinType::Char => "char",
                BuiltinType::Str => "str",
                BuiltinType::Error => "error",
                BuiltinType::Void | BuiltinType::Never => "core",
            };
            BuiltinTypeLocator::new(builtin, module(package, &[path]), builtin.spelling())
        })
        .collect()
}

fn structural_attachments(package: &PackageIdentity) -> Vec<StructuralAttachmentInput> {
    StructuralAttachment::ALL
        .iter()
        .copied()
        .map(|attachment| {
            let path = match attachment {
                StructuralAttachment::Slice => &["slice"][..],
            };
            StructuralAttachmentInput::new(attachment, module(package, path))
        })
        .collect()
}

fn standard_roles(package: &PackageIdentity) -> Vec<StandardRoleLocator> {
    StandardDeclarationRole::ALL
        .iter()
        .copied()
        .map(|role| {
            let (path, kind, name) = bundled_standard_source_location(role);
            StandardRoleLocator::new(role, module(package, path), kind, name)
        })
        .collect()
}

const fn bundled_standard_source_location(
    role: StandardDeclarationRole,
) -> (&'static [&'static str], NodeKind, &'static str) {
    use StandardDeclarationRole as Role;

    match role {
        Role::AbortingAllocator => (&["mem"], NodeKind::StructDeclaration, "Allocator"),
        Role::AllocationContext => (&["mem"], NodeKind::StructDeclaration, "AllocationContext"),
        Role::AllocationRequest => (&["mem"], NodeKind::FunctionDeclaration, "alloc_pages"),
        Role::OwnedString => (&["string"], NodeKind::StructDeclaration, "String"),
        Role::InterpolationConstructor => (&["string"], NodeKind::ConstructionFunction, "empty"),
        Role::InterpolationTextAppender => (&["string"], NodeKind::InherentMethod, "push_str"),
        Role::FormatInterface => (&["fmt"], NodeKind::InterfaceDeclaration, "Format"),
        Role::FormatMethod => (&["fmt"], NodeKind::InterfaceMethod, "format_into"),
        Role::IteratorInterface => (&["iter"], NodeKind::InterfaceDeclaration, "Iterator"),
        Role::IteratorItem => (&["iter"], NodeKind::AssociatedTypeDeclaration, "Item"),
        Role::IteratorNextMethod => (&["iter"], NodeKind::InterfaceMethod, "next"),
        Role::ExactSizeIteratorInterface => (
            &["iter"],
            NodeKind::InterfaceDeclaration,
            "ExactSizeIterator",
        ),
        Role::ExactSizeIteratorRemainingLenMethod => {
            (&["iter"], NodeKind::InterfaceMethod, "remaining_len")
        }
        Role::ProcessAbort => (&["process"], NodeKind::FunctionDeclaration, "abort"),
    }
}

fn primitive_roles(package: &PackageIdentity) -> Vec<PrimitiveRoleLocator> {
    PrimitiveRole::ALL
        .iter()
        .copied()
        .map(|role| {
            let (path, name) = bundled_primitive_source_location(role);
            PrimitiveRoleLocator::new(role, module(package, path), name)
        })
        .collect()
}

fn target_service_roles(package: &PackageIdentity) -> Vec<TargetServiceRoleLocator> {
    TargetServiceRole::ALL
        .iter()
        .copied()
        .map(|role| {
            let name = match role {
                TargetServiceRole::DarwinGetAddressInfo => "get_address_info_raw",
                TargetServiceRole::DarwinFreeAddressInfo => "free_address_info_raw",
            };
            TargetServiceRoleLocator::new(
                role,
                module(package, &["internal", "os", "darwin"]),
                name,
            )
        })
        .collect()
}

/// Returns the sole physical source location of a bundled standard primitive role.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive role-to-source table is the profile's reviewable authority"
)]
pub const fn bundled_primitive_source_location(
    role: PrimitiveRole,
) -> (&'static [&'static str], &'static str) {
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
        Role::MemoryMap => (&["mem"], "map_pages_raw"),
        Role::MemoryUnmap => (&["mem"], "unmap_pages_raw"),
        Role::DescriptorClose => (&["internal", "os", "darwin"], "close_descriptor"),
        Role::DatagramSocketOpen => (DARWIN_NET, "datagram_socket_open_raw"),
        Role::DatagramSocketConfigure => (DARWIN_NET, "datagram_socket_configure_raw"),
        Role::DatagramBind => (DARWIN_NET, "datagram_bind_raw"),
        Role::DatagramConnect => (DARWIN_NET, "datagram_connect_raw"),
        Role::DatagramConnectStatus => (DARWIN_NET, "datagram_connect_status_raw"),
        Role::DatagramSend => (DARWIN_NET, "datagram_send_raw"),
        Role::DatagramSendTo => (DARWIN_NET, "datagram_send_to_raw"),
        Role::DatagramReceive => (DARWIN_NET, "datagram_receive_raw"),
        Role::DatagramLocalAddress => (DARWIN_NET, "datagram_local_address_raw"),
        Role::DatagramPeerAddress => (DARWIN_NET, "datagram_peer_address_raw"),
        Role::EntropySeedFill => (&["internal", "hash"], "fill_seed_raw"),
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
        Role::CharacterFromU32Unchecked => (&["internal", "character"], "char_from_u32_unchecked"),
        Role::CharacterCodePoint => (&["internal", "character"], "char_code_point_raw"),
        Role::U8Truncate => (&["num"], "u8_truncate_raw"),
        Role::U16Truncate => (&["num"], "u16_truncate_raw"),
        Role::U32Truncate => (&["num"], "u32_truncate_raw"),
        Role::I8Truncate => (&["num"], "i8_truncate_raw"),
        Role::I16Truncate => (&["num"], "i16_truncate_raw"),
        Role::I32Truncate => (&["num"], "i32_truncate_raw"),
        Role::F32FromBits => (&["num"], "f32_from_bits_raw"),
        Role::F32ToBits => (&["num"], "f32_to_bits_raw"),
        Role::F64FromBits => (&["num"], "f64_from_bits_raw"),
        Role::F64ToBits => (&["num"], "f64_to_bits_raw"),
        Role::F32Floor => (&["num"], "f32_floor_raw"),
        Role::F32Ceil => (&["num"], "f32_ceil_raw"),
        Role::F32Trunc => (&["num"], "f32_trunc_raw"),
        Role::F32RoundTiesEven => (&["num"], "f32_round_ties_even_raw"),
        Role::F64Floor => (&["num"], "f64_floor_raw"),
        Role::F64Ceil => (&["num"], "f64_ceil_raw"),
        Role::F64Trunc => (&["num"], "f64_trunc_raw"),
        Role::F64RoundTiesEven => (&["num"], "f64_round_ties_even_raw"),
        Role::F64ToF32 => (&["num"], "f64_to_f32_raw"),
        Role::F64ToI64 => (&["num"], "f64_to_i64_raw"),
        Role::F64ToU64 => (&["num"], "f64_to_u64_raw"),
        Role::U64WrappingAdd => (&["num"], "u64_wrapping_add_raw"),
        Role::U64WrappingSubtract => (&["num"], "u64_wrapping_sub_raw"),
        Role::U64WrappingMultiply => (&["num"], "u64_wrapping_mul_raw"),
        Role::U64MultiplyHigh => (&["num"], "u64_mul_high_raw"),
        Role::U64BitwiseAnd => (&["num"], "u64_bit_and_raw"),
        Role::U64BitwiseOr => (&["num"], "u64_bit_or_raw"),
        Role::U64BitwiseXor => (&["num"], "u64_bit_xor_raw"),
        Role::U64RotateRight => (&["num"], "u64_rotate_right_raw"),
        Role::U64LeadingZeros => (&["num"], "u64_leading_zeros_raw"),
        Role::ProcessExit => (&["process"], "exit_raw"),
        Role::ProcessArgumentCount => (&["process"], "arg_count_raw"),
        Role::ProcessArgument => (&["process"], "arg_raw"),
        Role::ProcessEnvironmentCount => (&["process"], "env_count_raw"),
        Role::ProcessEnvironmentName => (&["process"], "env_name_raw"),
        Role::ProcessEnvironmentValue => (&["process"], "env_value_raw"),
        Role::MonotonicCounterRead => (&["internal", "time"], "monotonic_counter_raw"),
        Role::MonotonicCounterFrequency => (&["internal", "time"], "monotonic_frequency_raw"),
        Role::MonotonicCounterDelta => (&["internal", "time"], "monotonic_delta_raw"),
        Role::WallClockRead => (&["internal", "time"], "wall_clock_raw"),
        Role::TimeoutWait => (&["internal", "time"], "timeout_wait_raw"),
        Role::DescriptorReadiness => (INTERNAL_TASK, "descriptor_readiness_raw"),
        Role::DescriptorReadinessOrDeadline => (INTERNAL_TASK, READINESS_OR_DEADLINE),
        Role::MonotonicDeadline => (&["internal", "time"], "monotonic_deadline_raw"),
        Role::ProcessCompletion => (INTERNAL_TASK, "process_completion_raw"),
        Role::TaskJoin => (&["task"], "join"),
        Role::TaskRace => (&["task"], "race_raw"),
        Role::NetworkConnectionCreate => (DARWIN_NET, "network_connection_create_raw"),
        Role::NetworkConnectionCreateHost => (DARWIN_NET, "network_connection_create_host_raw"),
        Role::NetworkTlsConnectionCreate => (DARWIN_NET, "network_tls_connection_create_raw"),
        Role::NetworkTlsConnectionCreateHost => {
            (DARWIN_NET, "network_tls_connection_create_host_raw")
        }
        Role::NetworkTlsConnectionMatchesApplicationProtocol => (
            DARWIN_NET,
            "network_tls_connection_matches_application_protocol_raw",
        ),
        Role::NetworkConnectionStart => (DARWIN_NET, "network_connection_start_raw"),
        Role::NetworkConnectionEventDescriptor => (DARWIN_NET, NET_EVENT_DESCRIPTOR),
        Role::NetworkConnectionBeginReceive => (DARWIN_NET, NET_BEGIN_RECEIVE),
        Role::NetworkConnectionBeginSend => (DARWIN_NET, NET_BEGIN_SEND),
        Role::NetworkConnectionReceiveEvent => (DARWIN_NET, NET_RECEIVE_EVENT),
        Role::NetworkConnectionTryReceiveEvent => {
            (DARWIN_NET, "network_connection_try_receive_event_raw")
        }
        Role::NetworkConnectionCopyLocalAddress => (DARWIN_NET, NET_COPY_LOCAL_ADDRESS),
        Role::NetworkConnectionCopyRemoteAddress => (DARWIN_NET, NET_COPY_REMOTE_ADDRESS),
        Role::NetworkConnectionRequestCancel => (DARWIN_NET, NET_REQUEST_CANCEL),
        Role::NetworkConnectionReleaseBarrier => (DARWIN_NET, NET_RELEASE_BARRIER),
        Role::NetworkConnectionRelease => (DARWIN_NET, "network_connection_release_raw"),
        Role::NetworkConnectionDispose => (DARWIN_NET, "network_connection_dispose_raw"),
        Role::NetworkListenerCreate => (DARWIN_NET, "network_listener_create_raw"),
        Role::NetworkListenerStart => (DARWIN_NET, "network_listener_start_raw"),
        Role::NetworkListenerEventDescriptor => {
            (DARWIN_NET, "network_listener_event_descriptor_raw")
        }
        Role::NetworkListenerReceiveEvent => (DARWIN_NET, "network_listener_receive_event_raw"),
        Role::NetworkListenerTryReceiveEvent => {
            (DARWIN_NET, "network_listener_try_receive_event_raw")
        }
        Role::NetworkListenerPort => (DARWIN_NET, "network_listener_port_raw"),
        Role::NetworkListenerRequestCancel => (DARWIN_NET, "network_listener_request_cancel_raw"),
        Role::NetworkListenerReleaseBarrier => (DARWIN_NET, "network_listener_release_barrier_raw"),
        Role::NetworkListenerRelease => (DARWIN_NET, "network_listener_release_raw"),
        Role::NetworkListenerDispose => (DARWIN_NET, "network_listener_dispose_raw"),
        Role::Syscall0 => (&["internal", "os", "darwin"], "syscall0"),
        Role::SyscallPair0 => (&["internal", "os", "darwin"], "syscall_pair0"),
        Role::Syscall1 => (&["internal", "os", "darwin"], "syscall1"),
        Role::Syscall2 => (&["internal", "os", "darwin"], "syscall2"),
        Role::Syscall3 => (&["internal", "os", "darwin"], "syscall3"),
        Role::Syscall4 => (&["internal", "os", "darwin"], "syscall4"),
        Role::Syscall6 => (&["internal", "os", "darwin"], "syscall6"),
        Role::Trap => (&["internal", "os", "darwin"], "trap"),
        Role::Unreachable => (&["internal", "os", "darwin"], "unreachable"),
    }
}

fn module(package: &PackageIdentity, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(package.clone(), path.iter().copied())
}
