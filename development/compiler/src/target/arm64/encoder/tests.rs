use super::*;

#[test]
fn encodes_movz_w0_imm16() {
    let mut encoder = Encoder::new();

    encoder.emit_movz_w(WReg::W0, 42, MoveWideShift::Lsl0);

    assert_eq!(encoder.finish(), vec![0x40, 0x05, 0x80, 0x52]);
}

#[test]
fn encodes_movk_w0_imm16_lsl16() {
    let mut encoder = Encoder::new();

    encoder.emit_movk_w(WReg::W0, 0x1234, MoveWideShift::Lsl16);

    assert_eq!(encoder.finish(), vec![0x80, 0x46, 0xa2, 0x72]);
}

#[test]
fn encodes_movz_w16_imm16() {
    let mut encoder = Encoder::new();

    encoder.emit_movz_w(WReg::W16, 1, MoveWideShift::Lsl0);

    assert_eq!(encoder.finish(), vec![0x30, 0x00, 0x80, 0x52]);
}

#[test]
fn encodes_mov_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_mov_w(WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0xe0, 0x03, 0x01, 0x2a]);
}

#[test]
fn encodes_mov_x9_x0() {
    let mut encoder = Encoder::new();

    encoder.emit_mov_x(XReg::X9, XReg::X0);

    assert_eq!(encoder.finish(), vec![0xe9, 0x03, 0x00, 0xaa]);
}

#[test]
fn encodes_orr_x16_x16_x17() {
    let mut encoder = Encoder::new();

    encoder.emit_orr_x(XReg::X16, XReg::X16, XReg::X17);

    assert_eq!(encoder.finish(), vec![0x10, 0x02, 0x11, 0xaa]);
}

#[test]
fn encodes_add_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_add_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0x0b]);
}

#[test]
fn encodes_add_x16_x17_x19() {
    let mut encoder = Encoder::new();

    encoder.emit_add_x(XReg::X16, XReg::X17, XReg::X19);

    assert_eq!(encoder.finish(), vec![0x30, 0x02, 0x13, 0x8b]);
}

#[test]
fn encodes_adds_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_adds_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0x2b]);
}

#[test]
fn encodes_adds_x0_x0_x1() {
    let mut encoder = Encoder::new();

    encoder.emit_adds_x(XReg::X0, XReg::X0, XReg::X1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0xab]);
}

#[test]
fn encodes_sub_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_sub_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0x4b]);
}

#[test]
fn encodes_subs_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_subs_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0x6b]);
}

#[test]
fn encodes_subs_x0_x0_x1() {
    let mut encoder = Encoder::new();

    encoder.emit_subs_x(XReg::X0, XReg::X0, XReg::X1);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0xeb]);
}

#[test]
fn encodes_mul_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_mul_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x7c, 0x01, 0x1b]);
}

#[test]
fn encodes_mul_x0_x0_x1() {
    let mut encoder = Encoder::new();

    encoder.emit_mul_x(XReg::X0, XReg::X0, XReg::X1);

    assert_eq!(encoder.finish(), vec![0x00, 0x7c, 0x01, 0x9b]);
}

#[test]
fn encodes_sdiv_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_sdiv_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x0c, 0xc1, 0x1a]);
}

#[test]
fn encodes_udiv_x0_x0_x1() {
    let mut encoder = Encoder::new();

    encoder.emit_udiv_x(XReg::X0, XReg::X0, XReg::X1);

    assert_eq!(encoder.finish(), vec![0x00, 0x08, 0xc1, 0x9a]);
}

#[test]
fn encodes_udiv_w0_w0_w1() {
    let mut encoder = Encoder::new();

    encoder.emit_udiv_w(WReg::W0, WReg::W0, WReg::W1);

    assert_eq!(encoder.finish(), vec![0x00, 0x08, 0xc1, 0x1a]);
}

#[test]
fn encodes_lslv_w0_w16_w0() {
    let mut encoder = Encoder::new();

    encoder.emit_lslv_w(WReg::W0, WReg::W16, WReg::W0);

    assert_eq!(encoder.finish(), vec![0x00, 0x22, 0xc0, 0x1a]);
}

#[test]
fn encodes_lslv_x0_x16_x0() {
    let mut encoder = Encoder::new();

    encoder.emit_lslv_x(XReg::X0, XReg::X16, XReg::X0);

    assert_eq!(encoder.finish(), vec![0x00, 0x22, 0xc0, 0x9a]);
}

#[test]
fn encodes_lsrv_x0_x16_x0() {
    let mut encoder = Encoder::new();

    encoder.emit_lsrv_x(XReg::X0, XReg::X16, XReg::X0);

    assert_eq!(encoder.finish(), vec![0x00, 0x26, 0xc0, 0x9a]);
}

#[test]
fn encodes_lsrv_w0_w16_w0() {
    let mut encoder = Encoder::new();

    encoder.emit_lsrv_w(WReg::W0, WReg::W16, WReg::W0);

    assert_eq!(encoder.finish(), vec![0x00, 0x26, 0xc0, 0x1a]);
}

#[test]
fn encodes_asrv_w0_w16_w0() {
    let mut encoder = Encoder::new();

    encoder.emit_asrv_w(WReg::W0, WReg::W16, WReg::W0);

    assert_eq!(encoder.finish(), vec![0x00, 0x2a, 0xc0, 0x1a]);
}

#[test]
fn encodes_msub_w0_w2_w1_w0() {
    let mut encoder = Encoder::new();

    encoder.emit_msub_w(WReg::W0, WReg::W2, WReg::W1, WReg::W0);

    assert_eq!(encoder.finish(), vec![0x40, 0x80, 0x01, 0x1b]);
}

#[test]
fn encodes_msub_x0_x2_x1_x0() {
    let mut encoder = Encoder::new();

    encoder.emit_msub_x(XReg::X0, XReg::X2, XReg::X1, XReg::X0);

    assert_eq!(encoder.finish(), vec![0x40, 0x80, 0x01, 0x9b]);
}

#[test]
fn encodes_smull_x17_w16_w0() {
    let mut encoder = Encoder::new();

    encoder.emit_smull_x(XReg::X17, WReg::W16, WReg::W0);

    assert_eq!(encoder.finish(), vec![0x11, 0x7e, 0x20, 0x9b]);
}

#[test]
fn encodes_umulh_x17_x16_x0() {
    let mut encoder = Encoder::new();

    encoder.emit_umulh_x(XReg::X17, XReg::X16, XReg::X0);

    assert_eq!(encoder.finish(), vec![0x11, 0x7e, 0xc0, 0x9b]);
}

#[test]
fn encodes_sxtw_x16_w17() {
    let mut encoder = Encoder::new();

    encoder.emit_sxtw_x_w(XReg::X16, WReg::W17);

    assert_eq!(encoder.finish(), vec![0x30, 0x7e, 0x40, 0x93]);
}

#[test]
fn encodes_sub_sp_sp_imm() {
    let mut encoder = Encoder::new();

    encoder.emit_sub_sp_imm(32);

    assert_eq!(encoder.finish(), vec![0xff, 0x83, 0x00, 0xd1]);
}

#[test]
fn encodes_add_sp_sp_imm() {
    let mut encoder = Encoder::new();

    encoder.emit_add_sp_imm(32);

    assert_eq!(encoder.finish(), vec![0xff, 0x83, 0x00, 0x91]);
}

#[test]
fn encodes_add_x16_sp_imm() {
    let mut encoder = Encoder::new();

    encoder.emit_add_x_sp_imm(XReg::X16, 32);

    assert_eq!(encoder.finish(), vec![0xf0, 0x83, 0x00, 0x91]);
}

#[test]
fn encodes_add_x16_x17_imm() {
    let mut encoder = Encoder::new();

    encoder.emit_add_x_imm(XReg::X16, XReg::X17, 32);

    assert_eq!(encoder.finish(), vec![0x30, 0x82, 0x00, 0x91]);
}

#[test]
fn encodes_sub_sp_sp_shifted_imm() {
    let mut encoder = Encoder::new();

    encoder.emit_sub_sp_imm(4096);

    assert_eq!(encoder.finish(), vec![0xff, 0x07, 0x40, 0xd1]);
}

#[test]
fn encodes_cmp_w16_w17() {
    let mut encoder = Encoder::new();

    encoder.emit_cmp_w(WReg::W16, WReg::W17);

    assert_eq!(encoder.finish(), vec![0x1f, 0x02, 0x11, 0x6b]);
}

#[test]
fn encodes_cmp_w16_zero() {
    let mut encoder = Encoder::new();

    encoder.emit_cmp_w_zero(WReg::W16);

    assert_eq!(encoder.finish(), vec![0x1f, 0x02, 0x1f, 0x6b]);
}

#[test]
fn encodes_cmp_x17_x16() {
    let mut encoder = Encoder::new();

    encoder.emit_cmp_x(XReg::X17, XReg::X16);

    assert_eq!(encoder.finish(), vec![0x3f, 0x02, 0x10, 0xeb]);
}

#[test]
fn encodes_cmp_x17_zero() {
    let mut encoder = Encoder::new();

    encoder.emit_cmp_x_zero(XReg::X17);

    assert_eq!(encoder.finish(), vec![0x3f, 0x02, 0x1f, 0xeb]);
}

#[test]
fn encodes_lsl_x17_x17_8() {
    let mut encoder = Encoder::new();

    encoder.emit_lsl_x_imm(XReg::X17, XReg::X17, 8);

    assert_eq!(encoder.finish(), vec![0x31, 0xde, 0x78, 0xd3]);
}

#[test]
fn encodes_lsr_x17_x16_8() {
    let mut encoder = Encoder::new();

    encoder.emit_lsr_x_imm(XReg::X17, XReg::X16, 8);

    assert_eq!(encoder.finish(), vec![0x11, 0xfe, 0x48, 0xd3]);
}

#[test]
fn encodes_movz_x0_imm16() {
    let mut encoder = Encoder::new();

    encoder.emit_movz_x(XReg::X0, 2, MoveWideShift::Lsl0);

    assert_eq!(encoder.finish(), vec![0x40, 0x00, 0x80, 0xd2]);
}

#[test]
fn encodes_movk_x2_imm16_lsl48() {
    let mut encoder = Encoder::new();

    encoder.emit_movk_x(XReg::X2, 0x1234, MoveWideShift::Lsl48);

    assert_eq!(encoder.finish(), vec![0x82, 0x46, 0xe2, 0xf2]);
}

#[test]
fn encodes_adr_x1_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_adr_x(XReg::X1, 36);

    assert_eq!(encoder.finish(), vec![0x21, 0x01, 0x00, 0x10]);
}

#[test]
fn encodes_adr_x1_negative_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_adr_x(XReg::X1, -4);

    assert_eq!(encoder.finish(), vec![0xe1, 0xff, 0xff, 0x10]);
}

#[test]
fn encodes_str_x30_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_str_x_sp(XReg::X30, 24);

    assert_eq!(encoder.finish(), vec![0xfe, 0x0f, 0x00, 0xf9]);
}

#[test]
fn encodes_ldr_x30_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_ldr_x_sp(XReg::X30, 24);

    assert_eq!(encoder.finish(), vec![0xfe, 0x0f, 0x40, 0xf9]);
}

#[test]
fn encodes_str_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_str_w_sp(WReg::W9, 12);

    assert_eq!(encoder.finish(), vec![0xe9, 0x0f, 0x00, 0xb9]);
}

#[test]
fn encodes_strb_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_strb_w_sp(WReg::W9, 7);

    assert_eq!(encoder.finish(), vec![0xe9, 0x1f, 0x00, 0x39]);
}

#[test]
fn encodes_ldrb_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_ldrb_w_sp(WReg::W9, 7);

    assert_eq!(encoder.finish(), vec![0xe9, 0x1f, 0x40, 0x39]);
}

#[test]
fn encodes_strh_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_strh_w_sp(WReg::W9, 6);

    assert_eq!(encoder.finish(), vec![0xe9, 0x0f, 0x00, 0x79]);
}

#[test]
fn encodes_ldrh_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_ldrh_w_sp(WReg::W9, 6);

    assert_eq!(encoder.finish(), vec![0xe9, 0x0f, 0x40, 0x79]);
}

#[test]
fn encodes_ldr_w_local_sp_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_ldr_w_sp(WReg::W15, 28);

    assert_eq!(encoder.finish(), vec![0xef, 0x1f, 0x40, 0xb9]);
}

#[test]
fn encodes_str_x16_x8_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_str_x_imm(XReg::X16, XReg::X8, 16);

    assert_eq!(encoder.finish(), vec![0x10, 0x09, 0x00, 0xf9]);
}

#[test]
fn encodes_strb_w16_x8_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_strb_w_imm(WReg::W16, XReg::X8, 3);

    assert_eq!(encoder.finish(), vec![0x10, 0x0d, 0x00, 0x39]);
}

#[test]
fn encodes_strh_w16_x8_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_strh_w_imm(WReg::W16, XReg::X8, 2);

    assert_eq!(encoder.finish(), vec![0x10, 0x05, 0x00, 0x79]);
}

#[test]
fn encodes_ldrh_w16_x8_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_ldrh_w_imm(WReg::W16, XReg::X8, 2);

    assert_eq!(encoder.finish(), vec![0x10, 0x05, 0x40, 0x79]);
}

#[test]
fn encodes_ldrb_w0_x1_x2() {
    let mut encoder = Encoder::new();

    encoder.emit_ldrb_w_reg(WReg::W0, XReg::X1, XReg::X2);

    assert_eq!(encoder.finish(), vec![0x20, 0x68, 0x62, 0x38]);
}

#[test]
fn encodes_ldr_w0_x1_x2() {
    let mut encoder = Encoder::new();

    encoder.emit_ldr_w_reg(WReg::W0, XReg::X1, XReg::X2);

    assert_eq!(encoder.finish(), vec![0x20, 0x68, 0x62, 0xb8]);
}

#[test]
fn patches_adr_x1_offset() {
    let mut encoder = Encoder::new();
    encoder.emit_movz_x(XReg::X0, 2, MoveWideShift::Lsl0);
    let adr_offset = encoder.position();
    encoder.emit_adr_x(XReg::X1, 0);

    encoder.patch_adr_x(adr_offset, XReg::X1, 36);

    assert_eq!(
        encoder.finish(),
        vec![
            0x40, 0x00, 0x80, 0xd2, // movz x0, #2
            0x21, 0x01, 0x00, 0x10, // adr x1, #36
        ]
    );
}

#[test]
fn encodes_bl_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_bl(8);

    assert_eq!(encoder.finish(), vec![0x02, 0x00, 0x00, 0x94]);
}

#[test]
fn encodes_b_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b(8);

    assert_eq!(encoder.finish(), vec![0x02, 0x00, 0x00, 0x14]);
}

#[test]
fn encodes_b_eq_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Eq, 8);

    assert_eq!(encoder.finish(), vec![0x40, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_ne_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Ne, 8);

    assert_eq!(encoder.finish(), vec![0x41, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_cs_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Cs, 8);

    assert_eq!(encoder.finish(), vec![0x42, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_vc_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Vc, 8);

    assert_eq!(encoder.finish(), vec![0x47, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_cc_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Cc, 8);

    assert_eq!(encoder.finish(), vec![0x43, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_hi_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Hi, 8);

    assert_eq!(encoder.finish(), vec![0x48, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_lt_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Lt, 8);

    assert_eq!(encoder.finish(), vec![0x4b, 0x00, 0x00, 0x54]);
}

#[test]
fn encodes_b_ge_positive_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_b_cond(BranchCondition::Ge, 8);

    assert_eq!(encoder.finish(), vec![0x4a, 0x00, 0x00, 0x54]);
}

#[test]
fn patches_b_offset() {
    let mut encoder = Encoder::new();
    let branch_offset = encoder.position();
    encoder.emit_b(0);
    encoder.emit_ret();

    encoder.patch_b(branch_offset, 4);

    assert_eq!(
        encoder.finish(),
        vec![
            0x01, 0x00, 0x00, 0x14, // b +4
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn patches_b_cond_offset() {
    let mut encoder = Encoder::new();
    let branch_offset = encoder.position();
    encoder.emit_b_cond(BranchCondition::Eq, 0);
    encoder.emit_ret();

    encoder.patch_b_cond(branch_offset, BranchCondition::Ne, 4);

    assert_eq!(
        encoder.finish(),
        vec![
            0x21, 0x00, 0x00, 0x54, // b.ne +4
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn encodes_bl_negative_offset() {
    let mut encoder = Encoder::new();

    encoder.emit_bl(-4);

    assert_eq!(encoder.finish(), vec![0xff, 0xff, 0xff, 0x97]);
}

#[test]
fn patches_bl_offset() {
    let mut encoder = Encoder::new();
    let branch_offset = encoder.position();
    encoder.emit_bl(0);
    encoder.emit_ret();

    encoder.patch_bl(branch_offset, 4);

    assert_eq!(
        encoder.finish(),
        vec![
            0x01, 0x00, 0x00, 0x94, // bl +4
            0xc0, 0x03, 0x5f, 0xd6, // ret
        ]
    );
}

#[test]
fn encodes_ret() {
    let mut encoder = Encoder::new();

    encoder.emit_ret();

    assert_eq!(encoder.finish(), vec![0xc0, 0x03, 0x5f, 0xd6]);
}

#[test]
fn encodes_svc_imm16() {
    let mut encoder = Encoder::new();

    encoder.emit_svc(0x80);

    assert_eq!(encoder.finish(), vec![0x01, 0x10, 0x00, 0xd4]);
}

#[test]
fn encodes_brk_zero() {
    let mut encoder = Encoder::new();

    encoder.emit_brk(0);

    assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x20, 0xd4]);
}
