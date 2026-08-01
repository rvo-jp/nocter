use super::*;

pub(super) const STDERR_FILENO: u64 = 2;
pub(super) const FALLIBLE_REPORT_FRAME_SIZE: u32 = 32;
pub(super) const WRITE_LOOP_FRAME_SIZE: u32 = 32;
pub(super) const WRITE_LOOP_FD_OFFSET: u32 = 0;
pub(super) const WRITE_LOOP_POINTER_OFFSET: u32 = 8;
pub(super) const WRITE_LOOP_REMAINING_OFFSET: u32 = 16;
pub(super) const WRITE_UNEXPECTED_RESULT_ERRNO: u64 = 0xffff;
pub(super) const FALLIBLE_SUCCESS_PAYLOAD_REGISTER_COUNT: usize = 2;
pub(super) const DIRECT_AGGREGATE_REGISTER_WORD_COUNT: usize = 2;
pub(super) const WRITE_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.write_failed",
    message: b"write failed",
};
pub(super) const READ_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.read_failed",
    message: b"read failed",
};
pub(super) const OPEN_FAILURE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.open_failed",
    message: b"open failed",
};
pub(super) const IO_INTERRUPTED_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.interrupted",
    message: b"operation interrupted",
};
pub(super) const IO_WOULD_BLOCK_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.would_block",
    message: b"operation would block",
};
pub(super) const IO_NOT_FOUND_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.not_found",
    message: b"file not found",
};
pub(super) const IO_PERMISSION_DENIED_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.permission_denied",
    message: b"permission denied",
};
pub(super) const IO_INVALID_INPUT_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.invalid_input",
    message: b"invalid I/O input",
};
pub(super) const IO_BROKEN_PIPE_PAYLOAD: StaticErrorPayload = StaticErrorPayload {
    code: b"std.io.broken_pipe",
    message: b"broken pipe",
};
pub(super) const OPEN_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_ENOENT,
        payload: IO_NOT_FOUND_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_ENOTDIR,
        payload: IO_NOT_FOUND_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EPERM,
        payload: IO_PERMISSION_DENIED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EACCES,
        payload: IO_PERMISSION_DENIED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
];
pub(super) const READ_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_EINTR,
        payload: IO_INTERRUPTED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EAGAIN,
        payload: IO_WOULD_BLOCK_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EBADF,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EISDIR,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
];
pub(super) const WRITE_ERRNO_PAYLOADS: &[DarwinErrnoPayload] = &[
    DarwinErrnoPayload {
        errno: DARWIN_EINTR,
        payload: IO_INTERRUPTED_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EAGAIN,
        payload: IO_WOULD_BLOCK_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EBADF,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EFAULT,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EINVAL,
        payload: IO_INVALID_INPUT_PAYLOAD,
    },
    DarwinErrnoPayload {
        errno: DARWIN_EPIPE,
        payload: IO_BROKEN_PIPE_PAYLOAD,
    },
];
pub(super) const ADR_MIN_BYTE_OFFSET: i64 = -(1 << 20);
pub(super) const ADR_MAX_BYTE_OFFSET: i64 = (1 << 20) - 1;
pub(super) const BRANCH_MIN_BYTE_OFFSET: i64 = -(1 << 27);
pub(super) const BRANCH_MAX_BYTE_OFFSET: i64 = (1 << 27) - 4;
pub(super) const DARWIN_READ_SYSCALL: u32 = 0x0200_0003;
pub(super) const DARWIN_OPEN_SYSCALL: u32 = 0x0200_0005;
pub(super) const DARWIN_WRITE_SYSCALL: u32 = 0x0200_0004;
pub(super) const DARWIN_CLOSE_SYSCALL: u32 = 0x0200_0006;
pub(super) const DARWIN_EXIT_SYSCALL: u32 = 0x0200_0001;
pub(super) const DARWIN_SYSCALL_TRAP: u16 = 0x80;
pub(super) const DARWIN_EPERM: i32 = 1;
pub(super) const DARWIN_ENOENT: i32 = 2;
pub(super) const DARWIN_EINTR: i32 = 4;
pub(super) const DARWIN_EBADF: i32 = 9;
pub(super) const DARWIN_EACCES: i32 = 13;
pub(super) const DARWIN_EFAULT: i32 = 14;
pub(super) const DARWIN_ENOTDIR: i32 = 20;
pub(super) const DARWIN_EISDIR: i32 = 21;
pub(super) const DARWIN_EINVAL: i32 = 22;
pub(super) const DARWIN_EPIPE: i32 = 32;
pub(super) const DARWIN_EAGAIN: i32 = 35;
pub(super) const I32_BIT_WIDTH: i32 = 32;
pub(super) const USIZE_BIT_WIDTH: u64 = 64;
