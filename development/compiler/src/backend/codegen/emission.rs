use super::*;

pub(super) fn emit_mov_i32_to_w0(encoder: &mut Encoder, value: i32) {
    emit_mov_i32_to_w(encoder, WReg::W0, value);
}

pub(super) fn emit_mov_i32_to_w(encoder: &mut Encoder, register: WReg, value: i32) {
    emit_mov_u32_to_w(encoder, register, value as u32);
}

pub(super) fn emit_mov_u32_to_w(encoder: &mut Encoder, register: WReg, value: u32) {
    encoder.emit_movz_w(register, value as u16, MoveWideShift::Lsl0);

    let high = value >> 16;
    if high != 0 {
        encoder.emit_movk_w(register, high as u16, MoveWideShift::Lsl16);
    }
}

pub(super) fn emit_mov_u64_to_x(encoder: &mut Encoder, register: XReg, value: u64) {
    encoder.emit_movz_x(register, value as u16, MoveWideShift::Lsl0);

    for (shift, amount) in [
        (MoveWideShift::Lsl16, 16),
        (MoveWideShift::Lsl32, 32),
        (MoveWideShift::Lsl48, 48),
    ] {
        let chunk = (value >> amount) as u16;
        if chunk != 0 {
            encoder.emit_movk_x(register, chunk, shift);
        }
    }
}

pub(super) fn emit_darwin_exit_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_EXIT_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

pub(super) fn emit_darwin_write_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_WRITE_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

pub(super) fn emit_darwin_read_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_READ_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

pub(super) fn emit_darwin_open_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_OPEN_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

pub(super) fn emit_darwin_close_syscall(encoder: &mut Encoder) {
    emit_mov_u32_to_w(encoder, WReg::W16, DARWIN_CLOSE_SYSCALL);
    encoder.emit_svc(DARWIN_SYSCALL_TRAP);
}

pub(super) fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}
