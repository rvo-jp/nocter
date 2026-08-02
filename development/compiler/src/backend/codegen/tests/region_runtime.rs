use super::*;

const RELEASE_LOOP_FRAME_BYTES: u32 = 16;
const DARWIN_MUNMAP_SYSCALL: u32 = 0x0200_0049;

#[test]
fn region_release_preserves_the_next_mapping_across_munmap() {
    let module = IrModule::new(vec![Function {
        name: "main".to_string(),
        target: CallTarget::same_file("main"),
        return_type: Type::I32,
        instructions: vec![
            Instruction::RegionEnter {
                destination: UsizeLocation::Local(0),
            },
            Instruction::RegionRelease {
                state: UsizeValue::Location(UsizeLocation::Local(0)),
                parent_state: UsizeValue::Const(0),
                parent_kind: UsizeValue::Const(0),
            },
            set_return_i32(0),
            Instruction::Return,
        ],
    }]);

    let code = generate_arm64_darwin_entry(&module).unwrap();
    let expected = [
        encoded_sub_sp_imm(RELEASE_LOOP_FRAME_BYTES),
        encoded_str_x_sp(XReg::X2, 0),
    ]
    .concat()
    .into_iter()
    .chain(encoded_mov_u32_to_w(WReg::W16, DARWIN_MUNMAP_SYSCALL))
    .chain(encoded_svc(DARWIN_SYSCALL_TRAP))
    .chain(encoded_ldr_x_sp(XReg::X8, 0))
    .chain(encoded_add_sp_imm(RELEASE_LOOP_FRAME_BYTES))
    .collect::<Vec<_>>();

    assert!(
        contains_bytes(&code.text, &expected),
        "region release must not rely on a syscall preserving x2"
    );
}

fn encoded_sub_sp_imm(bytes: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_sub_sp_imm(bytes);
    encoded_instruction(encoder)
}

fn encoded_add_sp_imm(bytes: u32) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_add_sp_imm(bytes);
    encoded_instruction(encoder)
}

fn encoded_svc(immediate: u16) -> [u8; 4] {
    let mut encoder = Encoder::new();
    encoder.emit_svc(immediate);
    encoded_instruction(encoder)
}
