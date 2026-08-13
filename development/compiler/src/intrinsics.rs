//! Compiler-owned identities for primitive operations.
//!
//! Source names are recognized once at the resolver/lowering boundary. All
//! later dispatch uses this closed identity domain rather than string equality.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum IntrinsicId {
    Addr,
    AllocationAbortRaw,
    AppendRaw,
    ArgCountRaw,
    ArgRaw,
    BytesFromStr,
    CloseFdRaw,
    CopyPtrToPtr,
    CopyStrToPtr,
    CreateRaw,
    CurrentAllocatorKind,
    CurrentAllocatorState,
    DropValueAtPtr,
    EnvCountRaw,
    EnvNameRaw,
    EnvValueRaw,
    ExitRaw,
    FromAddr,
    FromRef,
    FromRefMut,
    OpenReadRaw,
    PointeeAlign,
    PointeeSize,
    ReadBytesRaw,
    SliceFromRawParts,
    SliceFromRawPartsMut,
    SliceFromRawPartsValue,
    SliceFromRawPartsValueMut,
    SliceLenRaw,
    SlicePtrAddrRaw,
    StoreU8ToPtr,
    StoreValueToPtr,
    TakeValueAtPtr,
    StrFromRawParts,
    StrLenRaw,
    StrPtrAddrRaw,
    StrSubviewUnchecked,
    Syscall(u8),
    Trap,
    Unreachable,
    WriteBytesRaw,
    WriteTextRaw,
}

impl IntrinsicId {
    pub(crate) fn from_source_name(name: &str) -> Option<Self> {
        Some(match name {
            "addr" => Self::Addr,
            "allocation_abort_raw" => Self::AllocationAbortRaw,
            "append_raw" => Self::AppendRaw,
            "arg_count_raw" => Self::ArgCountRaw,
            "arg_raw" => Self::ArgRaw,
            "bytes_from_str" => Self::BytesFromStr,
            "close_fd_raw" => Self::CloseFdRaw,
            "copy_ptr_to_ptr" => Self::CopyPtrToPtr,
            "copy_str_to_ptr" => Self::CopyStrToPtr,
            "create_raw" => Self::CreateRaw,
            "current_allocator_kind" => Self::CurrentAllocatorKind,
            "current_allocator_state" => Self::CurrentAllocatorState,
            "drop_value_at_ptr" => Self::DropValueAtPtr,
            "env_count_raw" => Self::EnvCountRaw,
            "env_name_raw" => Self::EnvNameRaw,
            "env_value_raw" => Self::EnvValueRaw,
            "exit_raw" => Self::ExitRaw,
            "from_addr" => Self::FromAddr,
            "from_ref" => Self::FromRef,
            "from_ref_mut" => Self::FromRefMut,
            "open_read_raw" => Self::OpenReadRaw,
            "pointee_align" => Self::PointeeAlign,
            "pointee_size" => Self::PointeeSize,
            "read_bytes_raw" => Self::ReadBytesRaw,
            "slice_from_raw_parts" => Self::SliceFromRawParts,
            "slice_from_raw_parts_mut" => Self::SliceFromRawPartsMut,
            "slice_from_raw_parts_value" => Self::SliceFromRawPartsValue,
            "slice_from_raw_parts_value_mut" => Self::SliceFromRawPartsValueMut,
            "slice_len_raw" => Self::SliceLenRaw,
            "slice_ptr_addr_raw" => Self::SlicePtrAddrRaw,
            "store_u8_to_ptr" => Self::StoreU8ToPtr,
            "store_value_to_ptr" => Self::StoreValueToPtr,
            "take_value_at_ptr" => Self::TakeValueAtPtr,
            "str_from_raw_parts" => Self::StrFromRawParts,
            "str_len_raw" => Self::StrLenRaw,
            "str_ptr_addr_raw" => Self::StrPtrAddrRaw,
            "str_subview_unchecked" => Self::StrSubviewUnchecked,
            "syscall0" => Self::Syscall(0),
            "syscall1" => Self::Syscall(1),
            "syscall2" => Self::Syscall(2),
            "syscall3" => Self::Syscall(3),
            "syscall4" => Self::Syscall(4),
            "syscall5" => Self::Syscall(5),
            "syscall6" => Self::Syscall(6),
            "trap" => Self::Trap,
            "unreachable" => Self::Unreachable,
            "write_bytes_raw" => Self::WriteBytesRaw,
            "write_text_raw" => Self::WriteTextRaw,
            _ => return None,
        })
    }

    pub(crate) const fn source_name(self) -> &'static str {
        match self {
            Self::Addr => "addr",
            Self::AllocationAbortRaw => "allocation_abort_raw",
            Self::AppendRaw => "append_raw",
            Self::ArgCountRaw => "arg_count_raw",
            Self::ArgRaw => "arg_raw",
            Self::BytesFromStr => "bytes_from_str",
            Self::CloseFdRaw => "close_fd_raw",
            Self::CopyPtrToPtr => "copy_ptr_to_ptr",
            Self::CopyStrToPtr => "copy_str_to_ptr",
            Self::CreateRaw => "create_raw",
            Self::CurrentAllocatorKind => "current_allocator_kind",
            Self::CurrentAllocatorState => "current_allocator_state",
            Self::DropValueAtPtr => "drop_value_at_ptr",
            Self::EnvCountRaw => "env_count_raw",
            Self::EnvNameRaw => "env_name_raw",
            Self::EnvValueRaw => "env_value_raw",
            Self::ExitRaw => "exit_raw",
            Self::FromAddr => "from_addr",
            Self::FromRef => "from_ref",
            Self::FromRefMut => "from_ref_mut",
            Self::OpenReadRaw => "open_read_raw",
            Self::PointeeAlign => "pointee_align",
            Self::PointeeSize => "pointee_size",
            Self::ReadBytesRaw => "read_bytes_raw",
            Self::SliceFromRawParts => "slice_from_raw_parts",
            Self::SliceFromRawPartsMut => "slice_from_raw_parts_mut",
            Self::SliceFromRawPartsValue => "slice_from_raw_parts_value",
            Self::SliceFromRawPartsValueMut => "slice_from_raw_parts_value_mut",
            Self::SliceLenRaw => "slice_len_raw",
            Self::SlicePtrAddrRaw => "slice_ptr_addr_raw",
            Self::StoreU8ToPtr => "store_u8_to_ptr",
            Self::StoreValueToPtr => "store_value_to_ptr",
            Self::TakeValueAtPtr => "take_value_at_ptr",
            Self::StrFromRawParts => "str_from_raw_parts",
            Self::StrLenRaw => "str_len_raw",
            Self::StrPtrAddrRaw => "str_ptr_addr_raw",
            Self::StrSubviewUnchecked => "str_subview_unchecked",
            Self::Syscall(0) => "syscall0",
            Self::Syscall(1) => "syscall1",
            Self::Syscall(2) => "syscall2",
            Self::Syscall(3) => "syscall3",
            Self::Syscall(4) => "syscall4",
            Self::Syscall(5) => "syscall5",
            Self::Syscall(6) => "syscall6",
            Self::Syscall(_) => "invalid_syscall",
            Self::Trap => "trap",
            Self::Unreachable => "unreachable",
            Self::WriteBytesRaw => "write_bytes_raw",
            Self::WriteTextRaw => "write_text_raw",
        }
    }
}

impl std::fmt::Display for IntrinsicId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.source_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_names_round_trip_through_identity() {
        for name in [
            "addr",
            "allocation_abort_raw",
            "slice_from_raw_parts_mut",
            "syscall0",
            "syscall6",
            "write_text_raw",
        ] {
            let intrinsic = IntrinsicId::from_source_name(name).unwrap();
            assert_eq!(intrinsic.source_name(), name);
        }
        assert_eq!(IntrinsicId::from_source_name("print"), None);
    }
}
