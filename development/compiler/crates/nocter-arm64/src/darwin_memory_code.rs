use crate::{
    Arm64BranchCondition, Arm64CodeBuilder, Arm64CodeError, Arm64DataSize, Arm64Instruction,
    Arm64NocterAbi,
};

const DARWIN_SUPERVISOR_CALL: u16 = 0x80;
const DARWIN_MUNMAP: u64 = 0x0200_0049;
const DARWIN_MMAP: u64 = 0x0200_00c5;
const READ_WRITE_PROTECTION: u64 = 3;
const PRIVATE_ANONYMOUS_MAPPING: u64 = 0x1002;

/// Emits one private anonymous mapping request.
///
/// The caller supplies the byte length in `x1`. The mapping pointer is returned in `x0`, and an
/// allocation failure terminates through the compiler-owned trap contract.
pub(crate) fn emit_map(code: &mut Arm64CodeBuilder) -> Result<(), Arm64CodeError> {
    crate::frame_access::load_immediate(code, argument(0), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(
        code,
        argument(2),
        READ_WRITE_PROTECTION,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(
        code,
        argument(3),
        PRIVATE_ANONYMOUS_MAPPING,
        Arm64DataSize::Bits64,
    );
    crate::frame_access::load_immediate(code, argument(4), u64::MAX, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(code, argument(5), 0, Arm64DataSize::Bits64);
    crate::frame_access::load_immediate(
        code,
        syscall_register(),
        DARWIN_MMAP,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: crate::runtime_trap::Arm64RuntimeTrap::AllocationFailure.immediate(),
    });
    code.bind(success)
}

/// Emits one mapping release request using the address in `x0` and byte length in `x1`.
pub(crate) fn emit_unmap(
    code: &mut Arm64CodeBuilder,
    failure: crate::runtime_trap::Arm64RuntimeTrap,
) -> Result<(), Arm64CodeError> {
    crate::frame_access::load_immediate(
        code,
        syscall_register(),
        DARWIN_MUNMAP,
        Arm64DataSize::Bits64,
    );
    code.append(Arm64Instruction::SupervisorCall {
        immediate: DARWIN_SUPERVISOR_CALL,
    });
    let success = code.create_label();
    code.branch_conditional(success, Arm64BranchCondition::CarryClear);
    code.append(Arm64Instruction::Break {
        immediate: failure.immediate(),
    });
    code.bind(success)
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("Darwin memory calls use ABI arguments")
}

fn syscall_register() -> crate::Arm64Register {
    Arm64NocterAbi::compiler_scratch_register(0)
        .expect("the ABI reserves a Darwin syscall-number register")
}
