use super::{BranchCondition, MoveWideShift, XReg};

pub(in crate::target::arm64::encoder) const MOVZ_W_BASE: u32 = 0x5280_0000;
pub(in crate::target::arm64::encoder) const MOVK_W_BASE: u32 = 0x7280_0000;
pub(in crate::target::arm64::encoder) const ORR_W_BASE: u32 = 0x2a00_0000;
pub(in crate::target::arm64::encoder) const ORR_X_BASE: u32 = 0xaa00_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const ADD_W_BASE: u32 = 0x0b00_0000;
pub(in crate::target::arm64::encoder) const ADD_X_BASE: u32 = 0x8b00_0000;
pub(in crate::target::arm64::encoder) const ADDS_W_BASE: u32 = 0x2b00_0000;
pub(in crate::target::arm64::encoder) const ADDS_X_BASE: u32 = 0xab00_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const SUB_W_BASE: u32 = 0x4b00_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const MADD_W_BASE: u32 = 0x1b00_0000;
pub(in crate::target::arm64::encoder) const MADD_X_BASE: u32 = 0x9b00_0000;
pub(in crate::target::arm64::encoder) const MSUB_W_BASE: u32 = 0x1b00_8000;
pub(in crate::target::arm64::encoder) const MSUB_X_BASE: u32 = 0x9b00_8000;
pub(in crate::target::arm64::encoder) const SMADDL_X_BASE: u32 = 0x9b20_0000;
pub(in crate::target::arm64::encoder) const UMULH_X_BASE: u32 = 0x9bc0_7c00;
pub(in crate::target::arm64::encoder) const SMULH_X_BASE: u32 = 0x9b40_7c00;
pub(in crate::target::arm64::encoder) const SXTW_X_BASE: u32 = 0x9340_7c00;
pub(in crate::target::arm64::encoder) const SDIV_W_BASE: u32 = 0x1ac0_0c00;
pub(in crate::target::arm64::encoder) const SDIV_X_BASE: u32 = 0x9ac0_0c00;
pub(in crate::target::arm64::encoder) const UDIV_W_BASE: u32 = 0x1ac0_0800;
pub(in crate::target::arm64::encoder) const UDIV_X_BASE: u32 = 0x9ac0_0800;
pub(in crate::target::arm64::encoder) const LSLV_W_BASE: u32 = 0x1ac0_2000;
pub(in crate::target::arm64::encoder) const LSLV_X_BASE: u32 = 0x9ac0_2000;
pub(in crate::target::arm64::encoder) const LSRV_W_BASE: u32 = 0x1ac0_2400;
pub(in crate::target::arm64::encoder) const LSRV_X_BASE: u32 = 0x9ac0_2400;
pub(in crate::target::arm64::encoder) const UBFM_X_BASE: u32 = 0xd340_0000;
pub(in crate::target::arm64::encoder) const BFM_X_BASE: u32 = 0xb340_0000;
pub(in crate::target::arm64::encoder) const ASRV_W_BASE: u32 = 0x1ac0_2800;
pub(in crate::target::arm64::encoder) const ASRV_X_BASE: u32 = 0x9ac0_2800;
pub(in crate::target::arm64::encoder) const SBFM_X_BASE: u32 = 0x9340_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const ADD_SP_IMM_BASE: u32 = 0x9100_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const SUB_SP_IMM_BASE: u32 = 0xd100_0000;
pub(in crate::target::arm64::encoder) const SUBS_W_BASE: u32 = 0x6b00_0000;
pub(in crate::target::arm64::encoder) const SUBS_W_IMM_BASE: u32 = 0x7100_0000;
pub(in crate::target::arm64::encoder) const SUBS_X_BASE: u32 = 0xeb00_0000;
pub(in crate::target::arm64::encoder) const MOVZ_X_BASE: u32 = 0xd280_0000;
pub(in crate::target::arm64::encoder) const MOVK_X_BASE: u32 = 0xf280_0000;
pub(in crate::target::arm64::encoder) const ADR_X_BASE: u32 = 0x1000_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const STR_W_SP_UNSIGNED_BASE: u32 = 0xb900_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const LDR_W_SP_UNSIGNED_BASE: u32 = 0xb940_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const STR_X_SP_UNSIGNED_BASE: u32 = 0xf900_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const LDR_X_SP_UNSIGNED_BASE: u32 = 0xf940_0000;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const STR_W_UNSIGNED_BASE: u32 = 0xb900_0000;
pub(in crate::target::arm64::encoder) const LDR_W_UNSIGNED_BASE: u32 = 0xb940_0000;
pub(in crate::target::arm64::encoder) const STRB_W_UNSIGNED_BASE: u32 = 0x3900_0000;
pub(in crate::target::arm64::encoder) const LDRB_W_UNSIGNED_BASE: u32 = 0x3940_0000;
pub(in crate::target::arm64::encoder) const STRH_W_UNSIGNED_BASE: u32 = 0x7900_0000;
pub(in crate::target::arm64::encoder) const LDRH_W_UNSIGNED_BASE: u32 = 0x7940_0000;
pub(in crate::target::arm64::encoder) const STR_X_UNSIGNED_BASE: u32 = 0xf900_0000;
pub(in crate::target::arm64::encoder) const LDR_X_UNSIGNED_BASE: u32 = 0xf940_0000;
pub(in crate::target::arm64::encoder) const LDRB_W_REG_BASE: u32 = 0x3860_6800;
pub(in crate::target::arm64::encoder) const LDR_W_REG_BASE: u32 = 0xb860_6800;
pub(in crate::target::arm64::encoder) const B_BASE: u32 = 0x1400_0000;
pub(in crate::target::arm64::encoder) const B_COND_BASE: u32 = 0x5400_0000;
pub(in crate::target::arm64::encoder) const BL_BASE: u32 = 0x9400_0000;
pub(in crate::target::arm64::encoder) const RET_X30: u32 = 0xd65f_03c0;
pub(in crate::target::arm64::encoder) const SVC_BASE: u32 = 0xd400_0001;
pub(in crate::target::arm64::encoder) const BRK_BASE: u32 = 0xd420_0000;

pub(in crate::target::arm64::encoder) const ADR_MIN_BYTE_OFFSET: i32 = -(1 << 20);
pub(in crate::target::arm64::encoder) const ADR_MAX_BYTE_OFFSET: i32 = (1 << 20) - 1;
pub(in crate::target::arm64::encoder) const BL_MIN_BYTE_OFFSET: i32 = -(1 << 27);
pub(in crate::target::arm64::encoder) const BL_MAX_BYTE_OFFSET: i32 = (1 << 27) - 4;
pub(in crate::target::arm64::encoder) const B_COND_MIN_BYTE_OFFSET: i32 = -(1 << 20);
pub(in crate::target::arm64::encoder) const B_COND_MAX_BYTE_OFFSET: i32 = (1 << 20) - 4;
#[allow(dead_code)]
pub(in crate::target::arm64::encoder) const SP_BITS: u32 = 31;
pub(in crate::target::arm64::encoder) const WZR_BITS: u32 = 31;
pub(in crate::target::arm64::encoder) const XZR_BITS: u32 = 31;

pub(in crate::target::arm64::encoder) const fn move_wide_fields(
    rd: u32,
    imm16: u16,
    shift: MoveWideShift,
) -> u32 {
    (shift.hw() << 21) | ((imm16 as u32) << 5) | rd
}

pub(in crate::target::arm64::encoder) fn adr_x_word(rd: XReg, byte_offset: i32) -> u32 {
    debug_assert!((ADR_MIN_BYTE_OFFSET..=ADR_MAX_BYTE_OFFSET).contains(&byte_offset));

    let encoded = (byte_offset as u32) & 0x001f_ffff;
    let immlo = encoded & 0x3;
    let immhi = (encoded >> 2) & 0x7ffff;
    ADR_X_BASE | (immlo << 29) | (immhi << 5) | rd.bits()
}

#[allow(dead_code)]
pub(in crate::target::arm64::encoder) fn add_sub_sp_imm_word(base: u32, byte_count: u32) -> u32 {
    let (shift, imm12) = if byte_count <= 0x0fff {
        (0, byte_count)
    } else {
        debug_assert_eq!(byte_count % 4096, 0);
        (1, byte_count / 4096)
    };
    debug_assert!(imm12 <= 0x0fff);

    base | (shift << 22) | (imm12 << 10) | (SP_BITS << 5) | SP_BITS
}

pub(in crate::target::arm64::encoder) fn add_x_sp_imm_word(rd: XReg, byte_count: u32) -> u32 {
    let (shift, imm12) = if byte_count <= 0x0fff {
        (0, byte_count)
    } else {
        debug_assert_eq!(byte_count % 4096, 0);
        (1, byte_count / 4096)
    };
    debug_assert!(imm12 <= 0x0fff);

    ADD_SP_IMM_BASE | (shift << 22) | (imm12 << 10) | (SP_BITS << 5) | rd.bits()
}

pub(in crate::target::arm64::encoder) fn add_x_imm_word(
    rd: XReg,
    rn: XReg,
    byte_count: u32,
) -> u32 {
    let (shift, imm12) = if byte_count <= 0x0fff {
        (0, byte_count)
    } else {
        debug_assert_eq!(byte_count % 4096, 0);
        (1, byte_count / 4096)
    };
    debug_assert!(imm12 <= 0x0fff);

    ADD_SP_IMM_BASE | (shift << 22) | (imm12 << 10) | (rn.bits() << 5) | rd.bits()
}

pub(in crate::target::arm64::encoder) fn lsl_x_imm_word(rd: XReg, rn: XReg, shift: u32) -> u32 {
    debug_assert!(shift < 64);
    let immr = (64 - shift) & 0x3f;
    let imms = 63 - shift;
    UBFM_X_BASE | (immr << 16) | (imms << 10) | (rn.bits() << 5) | rd.bits()
}

pub(in crate::target::arm64::encoder) fn lsr_x_imm_word(rd: XReg, rn: XReg, shift: u32) -> u32 {
    debug_assert!(shift < 64);
    UBFM_X_BASE | (shift << 16) | (0x3f << 10) | (rn.bits() << 5) | rd.bits()
}

#[allow(dead_code)]
pub(in crate::target::arm64::encoder) fn load_store_sp_word(
    base: u32,
    rt: u32,
    byte_offset: u32,
    access_size: u32,
) -> u32 {
    load_store_unsigned_word(base, rt, SP_BITS, byte_offset, access_size)
}

pub(in crate::target::arm64::encoder) fn load_store_unsigned_word(
    base: u32,
    rt: u32,
    rn: u32,
    byte_offset: u32,
    access_size: u32,
) -> u32 {
    debug_assert_eq!(byte_offset % access_size, 0);
    let scaled_offset = byte_offset / access_size;
    debug_assert!(scaled_offset <= 0x0fff);

    base | (scaled_offset << 10) | (rn << 5) | rt
}

pub(in crate::target::arm64::encoder) fn bl_word(byte_offset: i32) -> u32 {
    debug_assert!((BL_MIN_BYTE_OFFSET..=BL_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    BL_BASE | (((byte_offset / 4) as u32) & 0x03ff_ffff)
}

pub(in crate::target::arm64::encoder) fn b_word(byte_offset: i32) -> u32 {
    debug_assert!((BL_MIN_BYTE_OFFSET..=BL_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    B_BASE | (((byte_offset / 4) as u32) & 0x03ff_ffff)
}

pub(in crate::target::arm64::encoder) fn b_cond_word(
    condition: BranchCondition,
    byte_offset: i32,
) -> u32 {
    debug_assert!((B_COND_MIN_BYTE_OFFSET..=B_COND_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    B_COND_BASE | ((((byte_offset / 4) as u32) & 0x0007_ffff) << 5) | condition.bits()
}
