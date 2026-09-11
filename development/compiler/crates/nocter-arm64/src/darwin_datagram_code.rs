use crate::{
    Arm64AddSubtract, Arm64AddSubtractDestination, Arm64BaseRegister, Arm64BranchCondition,
    Arm64CodeBuilder, Arm64DataSize, Arm64Instruction, Arm64MaterializationError, Arm64NocterAbi,
};

/// One compiler-owned Darwin datagram operation selected from a closed primitive role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DarwinDatagramOperation {
    SocketOpen,
    SocketConfigure,
    Bind,
    Connect,
    ConnectStatus,
    Send,
    SendTo,
    Receive,
    LocalAddress,
    PeerAddress,
}

impl DarwinDatagramOperation {
    /// Every closed datagram operation in selection order.
    pub const ALL: &'static [Self] = &[
        Self::SocketOpen,
        Self::SocketConfigure,
        Self::Bind,
        Self::Connect,
        Self::ConnectStatus,
        Self::Send,
        Self::SendTo,
        Self::Receive,
        Self::LocalAddress,
        Self::PeerAddress,
    ];
}

/// Materializes a closed datagram operation over the ordinary Nocter argument/result registers.
pub(crate) fn emit(
    operation: DarwinDatagramOperation,
    code: &mut Arm64CodeBuilder,
) -> Result<(), Arm64MaterializationError> {
    match operation {
        DarwinDatagramOperation::SocketOpen => emit_socket_open(code),
        DarwinDatagramOperation::SocketConfigure => emit_socket_configure(code),
        DarwinDatagramOperation::Bind => emit_direct(code, DarwinCall::Bind),
        DarwinDatagramOperation::Connect => emit_direct(code, DarwinCall::Connect),
        DarwinDatagramOperation::ConnectStatus => emit_connect_status(code),
        DarwinDatagramOperation::Send => emit_send(code),
        DarwinDatagramOperation::SendTo => emit_send_to(code),
        DarwinDatagramOperation::Receive => emit_receive(code),
        DarwinDatagramOperation::LocalAddress => emit_direct(code, DarwinCall::LocalAddress),
        DarwinDatagramOperation::PeerAddress => emit_direct(code, DarwinCall::PeerAddress),
    }
}

#[derive(Clone, Copy)]
enum DarwinCall {
    Bind,
    Connect,
    LocalAddress,
    PeerAddress,
}

fn emit_socket_open(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    let ipv4 = code.create_label();
    let family_ready = code.create_label();
    compare_zero(code, argument(0));
    code.branch_conditional(ipv4, Arm64BranchCondition::Equal);
    immediate(
        code,
        argument(0),
        crate::darwin_kernel_abi::DarwinDatagramAbi::IPV6_FAMILY,
    );
    code.branch(family_ready, false);
    code.bind(ipv4)?;
    immediate(
        code,
        argument(0),
        crate::darwin_kernel_abi::DarwinDatagramAbi::IPV4_FAMILY,
    );
    code.bind(family_ready)?;
    immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDatagramAbi::DATAGRAM_SOCKET,
    );
    immediate(code, argument(2), 0);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Socket,
    );
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_socket_configure(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    // `x17` is excluded from virtual allocation and remains untouched by Darwin syscall entry.
    // It preserves the descriptor while `x0..x4` are repeatedly prepared for fixed operations.
    let descriptor = Arm64NocterAbi::compiler_scratch_register(1)
        .ok_or(Arm64MaterializationError::MissingScratchRegister)?;
    move_register(code, argument(0), descriptor);
    move_register(code, argument(2), argument(3));

    let common = code.create_label();
    let finish = code.create_label();
    compare_zero(code, argument(1));
    code.branch_conditional(common, Arm64BranchCondition::Equal);
    move_register(code, descriptor, argument(0));
    immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDatagramAbi::IPV6_LEVEL,
    );
    immediate(
        code,
        argument(2),
        crate::darwin_kernel_abi::DarwinDatagramAbi::IPV6_ONLY,
    );
    immediate(
        code,
        argument(4),
        crate::darwin_kernel_abi::DarwinDatagramAbi::INTEGER_OPTION_SIZE,
    );
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::SetSocketOption,
    );
    code.branch_conditional(finish, Arm64BranchCondition::CarrySet);

    code.bind(common)?;
    emit_fcntl(
        code,
        descriptor,
        crate::darwin_kernel_abi::DarwinDatagramAbi::SET_DESCRIPTOR_FLAGS,
        crate::darwin_kernel_abi::DarwinDatagramAbi::CLOSE_ON_EXEC,
    );
    code.branch_conditional(finish, Arm64BranchCondition::CarrySet);
    emit_fcntl(
        code,
        descriptor,
        crate::darwin_kernel_abi::DarwinDatagramAbi::SET_NO_SIGPIPE,
        1,
    );
    code.branch_conditional(finish, Arm64BranchCondition::CarrySet);
    emit_fcntl(
        code,
        descriptor,
        crate::darwin_kernel_abi::DarwinDatagramAbi::SET_STATUS_FLAGS,
        crate::darwin_kernel_abi::DarwinDatagramAbi::NONBLOCKING,
    );
    code.bind(finish)?;
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_fcntl(
    code: &mut Arm64CodeBuilder,
    descriptor: crate::Arm64Register,
    command: u64,
    value: u64,
) {
    move_register(code, descriptor, argument(0));
    immediate(code, argument(1), command);
    immediate(code, argument(2), value);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::Fcntl,
    );
}

fn emit_direct(
    code: &mut Arm64CodeBuilder,
    call: DarwinCall,
) -> Result<(), Arm64MaterializationError> {
    let call = match call {
        DarwinCall::Bind => crate::darwin_kernel_abi::DarwinSystemCall::Bind,
        DarwinCall::Connect => crate::darwin_kernel_abi::DarwinSystemCall::Connect,
        DarwinCall::LocalAddress => crate::darwin_kernel_abi::DarwinSystemCall::GetSocketName,
        DarwinCall::PeerAddress => crate::darwin_kernel_abi::DarwinSystemCall::GetPeerName,
    };
    crate::darwin_kernel_abi::emit_system_call(code, call);
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_connect_status(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    move_register(code, argument(2), argument(4));
    move_register(code, argument(1), argument(3));
    immediate(
        code,
        argument(1),
        crate::darwin_kernel_abi::DarwinDatagramAbi::SOCKET_LEVEL,
    );
    immediate(
        code,
        argument(2),
        crate::darwin_kernel_abi::DarwinDatagramAbi::SOCKET_ERROR,
    );
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::GetSocketOption,
    );
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_send(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    immediate(code, argument(3), 0);
    immediate(code, argument(4), 0);
    immediate(code, argument(5), 0);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::SendTo,
    );
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_send_to(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    move_register(code, argument(4), argument(5));
    move_register(code, argument(3), argument(4));
    immediate(code, argument(3), 0);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::SendTo,
    );
    crate::system_primitive_code::emit_system_call_result(code)
}

fn emit_receive(code: &mut Arm64CodeBuilder) -> Result<(), Arm64MaterializationError> {
    immediate(code, argument(2), 0);
    crate::darwin_kernel_abi::emit_system_call(
        code,
        crate::darwin_kernel_abi::DarwinSystemCall::ReceiveMessage,
    );
    crate::system_primitive_code::emit_system_call_result(code)
}

fn compare_zero(code: &mut Arm64CodeBuilder, value: crate::Arm64Register) {
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Subtract,
        set_flags: true,
        destination: Arm64AddSubtractDestination::Zero,
        source: Arm64BaseRegister::General(value),
        immediate: 0,
        shift_12: false,
    });
}

fn immediate(code: &mut Arm64CodeBuilder, destination: crate::Arm64Register, value: u64) {
    crate::frame_access::load_immediate(code, destination, value, Arm64DataSize::Bits64);
}

fn move_register(
    code: &mut Arm64CodeBuilder,
    source: crate::Arm64Register,
    destination: crate::Arm64Register,
) {
    if source == destination {
        return;
    }
    code.append(Arm64Instruction::AddSubtractImmediate {
        size: Arm64DataSize::Bits64,
        operation: Arm64AddSubtract::Add,
        set_flags: false,
        destination: Arm64AddSubtractDestination::General(destination),
        source: Arm64BaseRegister::General(source),
        immediate: 0,
        shift_12: false,
    });
}

fn argument(index: u8) -> crate::Arm64Register {
    Arm64NocterAbi::argument_register(index).expect("Darwin datagram calls use ABI arguments")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_closed_datagram_operation_materializes_complete_code() {
        for operation in DarwinDatagramOperation::ALL {
            let mut builder = Arm64CodeBuilder::new();
            emit(*operation, &mut builder).unwrap();
            let code = builder.finish().unwrap();
            assert!(code.instruction_count() > 0, "{operation:?}");
            assert_eq!(
                system_call_count(code.bytes()),
                if *operation == DarwinDatagramOperation::SocketConfigure {
                    4
                } else {
                    1
                },
                "{operation:?}",
            );
        }
    }

    fn system_call_count(bytes: &[u8]) -> usize {
        let instruction = Arm64Instruction::SupervisorCall { immediate: 0x80 }
            .encode()
            .unwrap();
        bytes
            .chunks_exact(instruction.len())
            .filter(|candidate| *candidate == instruction)
            .count()
    }
}
