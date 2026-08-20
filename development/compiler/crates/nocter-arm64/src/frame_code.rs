use crate::{
    Arm64AddSubtract, Arm64CodeBuilder, Arm64FrameLayout, Arm64LoadStoreSize, Arm64NocterAbi,
    Arm64Register,
};

/// Materializes one validated fixed-frame layout without imposing an immediate-size frame limit.
pub struct Arm64FrameCode;

impl Arm64FrameCode {
    pub fn emit_prologue(frame: &Arm64FrameLayout, code: &mut Arm64CodeBuilder) {
        crate::frame_access::adjust_stack(code, frame.size(), Arm64AddSubtract::Subtract);
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            frame_pointer(),
            frame.frame_record_offset(),
        );
        crate::frame_access::store_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            link_register(),
            frame.frame_record_offset() + 8,
        );
        for saved in frame.saved_registers() {
            crate::frame_access::store_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                saved.register(),
                saved.offset(),
            );
        }
        crate::frame_access::form_stack_address(code, frame_pointer(), frame.frame_record_offset());
    }

    pub fn emit_epilogue(frame: &Arm64FrameLayout, code: &mut Arm64CodeBuilder) {
        for saved in frame.saved_registers() {
            crate::frame_access::load_at_stack_offset(
                code,
                Arm64LoadStoreSize::Double,
                saved.register(),
                saved.offset(),
            );
        }
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            link_register(),
            frame.frame_record_offset() + 8,
        );
        crate::frame_access::load_at_stack_offset(
            code,
            Arm64LoadStoreSize::Double,
            frame_pointer(),
            frame.frame_record_offset(),
        );
        crate::frame_access::adjust_stack(code, frame.size(), Arm64AddSubtract::Add);
        code.append(crate::Arm64Instruction::Return {
            target: link_register(),
        });
    }
}

fn frame_pointer() -> Arm64Register {
    Arm64NocterAbi::frame_pointer_register()
}

fn link_register() -> Arm64Register {
    Arm64NocterAbi::link_register()
}
