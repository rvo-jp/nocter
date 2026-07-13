#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn emit_movz_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        debug_assert!(shift.is_valid_for_wide_32());
        self.emit_word(MOVZ_W_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_movk_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        debug_assert!(shift.is_valid_for_wide_32());
        self.emit_word(MOVK_W_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_mov_w(&mut self, rd: WReg, rm: WReg) {
        self.emit_word(ORR_W_BASE | (rm.bits() << 16) | (WZR_BITS << 5) | rd.bits());
    }

    pub(crate) fn emit_mov_x(&mut self, rd: XReg, rm: XReg) {
        self.emit_word(ORR_X_BASE | (rm.bits() << 16) | (XZR_BITS << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_add_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ADD_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_adds_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ADDS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_adds_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(ADDS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_sub_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SUB_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_subs_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SUBS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_subs_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(SUBS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_mul_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(
            MADD_W_BASE | (rm.bits() << 16) | (WZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_mul_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(
            MADD_X_BASE | (rm.bits() << 16) | (XZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_sdiv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(SDIV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_udiv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(UDIV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lslv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(LSLV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lslv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(LSLV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_lsrv_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(LSRV_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_asrv_w(&mut self, rd: WReg, rn: WReg, rm: WReg) {
        self.emit_word(ASRV_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_msub_w(&mut self, rd: WReg, rn: WReg, rm: WReg, ra: WReg) {
        self.emit_word(
            MSUB_W_BASE | (rm.bits() << 16) | (ra.bits() << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_msub_x(&mut self, rd: XReg, rn: XReg, rm: XReg, ra: XReg) {
        self.emit_word(
            MSUB_X_BASE | (rm.bits() << 16) | (ra.bits() << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_smull_x(&mut self, rd: XReg, rn: WReg, rm: WReg) {
        self.emit_word(
            SMADDL_X_BASE | (rm.bits() << 16) | (WZR_BITS << 10) | (rn.bits() << 5) | rd.bits(),
        );
    }

    pub(crate) fn emit_umulh_x(&mut self, rd: XReg, rn: XReg, rm: XReg) {
        self.emit_word(UMULH_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rd.bits());
    }

    pub(crate) fn emit_sxtw_x_w(&mut self, rd: XReg, rn: WReg) {
        self.emit_word(SXTW_X_BASE | (rn.bits() << 5) | rd.bits());
    }

    #[allow(dead_code)]
    pub(crate) fn emit_sub_sp_imm(&mut self, byte_count: u32) {
        self.emit_word(add_sub_sp_imm_word(SUB_SP_IMM_BASE, byte_count));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_add_sp_imm(&mut self, byte_count: u32) {
        self.emit_word(add_sub_sp_imm_word(ADD_SP_IMM_BASE, byte_count));
    }

    pub(crate) fn emit_add_x_sp_imm(&mut self, rd: XReg, byte_count: u32) {
        self.emit_word(add_x_sp_imm_word(rd, byte_count));
    }

    pub(crate) fn emit_cmp_w(&mut self, rn: WReg, rm: WReg) {
        self.emit_word(SUBS_W_BASE | (rm.bits() << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_w_zero(&mut self, rn: WReg) {
        self.emit_word(SUBS_W_BASE | (WZR_BITS << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_x(&mut self, rn: XReg, rm: XReg) {
        self.emit_word(SUBS_X_BASE | (rm.bits() << 16) | (rn.bits() << 5) | WZR_BITS);
    }

    pub(crate) fn emit_cmp_x_zero(&mut self, rn: XReg) {
        self.emit_word(SUBS_X_BASE | (XZR_BITS << 16) | (rn.bits() << 5) | XZR_BITS);
    }

    pub(crate) fn emit_movz_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVZ_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_movk_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVK_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_adr_x(&mut self, rd: XReg, byte_offset: i32) {
        self.emit_word(adr_x_word(rd, byte_offset));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_str_x_sp(&mut self, rt: XReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STR_X_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            8,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_ldr_x_sp(&mut self, rt: XReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDR_X_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            8,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_str_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STR_W_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            4,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_ldr_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDR_W_SP_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_strb_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            STRB_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_ldrb_w_sp(&mut self, rt: WReg, byte_offset: u32) {
        self.emit_word(load_store_sp_word(
            LDRB_W_UNSIGNED_BASE,
            rt.bits(),
            byte_offset,
            1,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn emit_str_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STR_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_ldr_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDR_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            4,
        ));
    }

    pub(crate) fn emit_strb_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STRB_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_ldrb_w_imm(&mut self, rt: WReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDRB_W_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            1,
        ));
    }

    pub(crate) fn emit_str_x_imm(&mut self, rt: XReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            STR_X_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            8,
        ));
    }

    pub(crate) fn emit_ldr_x_imm(&mut self, rt: XReg, rn: XReg, byte_offset: u32) {
        self.emit_word(load_store_unsigned_word(
            LDR_X_UNSIGNED_BASE,
            rt.bits(),
            rn.bits(),
            byte_offset,
            8,
        ));
    }

    pub(crate) fn emit_ldrb_w_reg(&mut self, rt: WReg, rn: XReg, rm: XReg) {
        self.emit_word(LDRB_W_REG_BASE | (rm.bits() << 16) | (rn.bits() << 5) | rt.bits());
    }

    pub(crate) fn emit_b(&mut self, byte_offset: i32) {
        self.emit_word(b_word(byte_offset));
    }

    pub(crate) fn emit_b_cond(&mut self, condition: BranchCondition, byte_offset: i32) {
        self.emit_word(b_cond_word(condition, byte_offset));
    }

    pub(crate) fn emit_bl(&mut self, byte_offset: i32) {
        self.emit_word(bl_word(byte_offset));
    }

    pub(crate) fn emit_ret(&mut self) {
        self.emit_word(RET_X30);
    }

    pub(crate) fn emit_svc(&mut self, imm16: u16) {
        self.emit_word(SVC_BASE | ((imm16 as u32) << 5));
    }

    pub(crate) fn emit_brk(&mut self, imm16: u16) {
        self.emit_word(BRK_BASE | ((imm16 as u32) << 5));
    }

    pub(crate) fn position(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn patch_adr_x(&mut self, instruction_offset: usize, rd: XReg, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = adr_x_word(rd, byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_bl(&mut self, instruction_offset: usize, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = bl_word(byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_b(&mut self, instruction_offset: usize, byte_offset: i32) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = b_word(byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn patch_b_cond(
        &mut self,
        instruction_offset: usize,
        condition: BranchCondition,
        byte_offset: i32,
    ) {
        debug_assert_eq!(instruction_offset % 4, 0);
        debug_assert!(instruction_offset + 4 <= self.bytes.len());

        let word = b_cond_word(condition, byte_offset);
        self.bytes[instruction_offset..instruction_offset + 4].copy_from_slice(&word.to_le_bytes());
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn emit_word(&mut self, word: u32) {
        self.bytes.extend_from_slice(&word.to_le_bytes());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WReg {
    W0,
    W1,
    W2,
    W3,
    W4,
    W5,
    W6,
    W7,
    W9,
    W10,
    W11,
    W12,
    W13,
    W14,
    W15,
    W16,
    W17,
}

impl WReg {
    pub(crate) fn argument(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::W0),
            1 => Some(Self::W1),
            2 => Some(Self::W2),
            3 => Some(Self::W3),
            4 => Some(Self::W4),
            5 => Some(Self::W5),
            6 => Some(Self::W6),
            7 => Some(Self::W7),
            _ => None,
        }
    }

    pub(crate) fn local(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::W9),
            1 => Some(Self::W10),
            2 => Some(Self::W11),
            3 => Some(Self::W12),
            4 => Some(Self::W13),
            5 => Some(Self::W14),
            6 => Some(Self::W15),
            _ => None,
        }
    }

    const fn bits(self) -> u32 {
        match self {
            Self::W0 => 0,
            Self::W1 => 1,
            Self::W2 => 2,
            Self::W3 => 3,
            Self::W4 => 4,
            Self::W5 => 5,
            Self::W6 => 6,
            Self::W7 => 7,
            Self::W9 => 9,
            Self::W10 => 10,
            Self::W11 => 11,
            Self::W12 => 12,
            Self::W13 => 13,
            Self::W14 => 14,
            Self::W15 => 15,
            Self::W16 => 16,
            Self::W17 => 17,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BranchCondition {
    Eq,
    Ne,
    Cs,
    Cc,
    Vc,
    Hi,
    Ls,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BranchCondition {
    const fn bits(self) -> u32 {
        match self {
            Self::Eq => 0,
            Self::Ne => 1,
            Self::Cs => 2,
            Self::Cc => 3,
            Self::Vc => 7,
            Self::Hi => 8,
            Self::Ls => 9,
            Self::Ge => 10,
            Self::Lt => 11,
            Self::Gt => 12,
            Self::Le => 13,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XReg {
    X0,
    X1,
    X2,
    X3,
    X4,
    X5,
    X6,
    X7,
    X8,
    X9,
    X10,
    X11,
    X12,
    X13,
    X14,
    X15,
    X16,
    X17,
    #[allow(dead_code)]
    X30,
}

impl XReg {
    pub(crate) fn argument(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::X0),
            1 => Some(Self::X1),
            2 => Some(Self::X2),
            3 => Some(Self::X3),
            4 => Some(Self::X4),
            5 => Some(Self::X5),
            6 => Some(Self::X6),
            7 => Some(Self::X7),
            _ => None,
        }
    }

    pub(crate) fn local(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::X9),
            1 => Some(Self::X10),
            2 => Some(Self::X11),
            3 => Some(Self::X12),
            4 => Some(Self::X13),
            5 => Some(Self::X14),
            6 => Some(Self::X15),
            _ => None,
        }
    }

    const fn bits(self) -> u32 {
        match self {
            Self::X0 => 0,
            Self::X1 => 1,
            Self::X2 => 2,
            Self::X3 => 3,
            Self::X4 => 4,
            Self::X5 => 5,
            Self::X6 => 6,
            Self::X7 => 7,
            Self::X8 => 8,
            Self::X9 => 9,
            Self::X10 => 10,
            Self::X11 => 11,
            Self::X12 => 12,
            Self::X13 => 13,
            Self::X14 => 14,
            Self::X15 => 15,
            Self::X16 => 16,
            Self::X17 => 17,
            Self::X30 => 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveWideShift {
    Lsl0,
    Lsl16,
    Lsl32,
    Lsl48,
}

impl MoveWideShift {
    const fn hw(self) -> u32 {
        match self {
            Self::Lsl0 => 0,
            Self::Lsl16 => 1,
            Self::Lsl32 => 2,
            Self::Lsl48 => 3,
        }
    }

    const fn is_valid_for_wide_32(self) -> bool {
        matches!(self, Self::Lsl0 | Self::Lsl16)
    }
}

const MOVZ_W_BASE: u32 = 0x5280_0000;
const MOVK_W_BASE: u32 = 0x7280_0000;
const ORR_W_BASE: u32 = 0x2a00_0000;
const ORR_X_BASE: u32 = 0xaa00_0000;
#[allow(dead_code)]
const ADD_W_BASE: u32 = 0x0b00_0000;
const ADDS_W_BASE: u32 = 0x2b00_0000;
const ADDS_X_BASE: u32 = 0xab00_0000;
#[allow(dead_code)]
const SUB_W_BASE: u32 = 0x4b00_0000;
#[allow(dead_code)]
const MADD_W_BASE: u32 = 0x1b00_0000;
const MADD_X_BASE: u32 = 0x9b00_0000;
const MSUB_W_BASE: u32 = 0x1b00_8000;
const MSUB_X_BASE: u32 = 0x9b00_8000;
const SMADDL_X_BASE: u32 = 0x9b20_0000;
const UMULH_X_BASE: u32 = 0x9bc0_7c00;
const SXTW_X_BASE: u32 = 0x9340_7c00;
const SDIV_W_BASE: u32 = 0x1ac0_0c00;
const UDIV_X_BASE: u32 = 0x9ac0_0800;
const LSLV_W_BASE: u32 = 0x1ac0_2000;
const LSLV_X_BASE: u32 = 0x9ac0_2000;
const LSRV_X_BASE: u32 = 0x9ac0_2400;
const ASRV_W_BASE: u32 = 0x1ac0_2800;
#[allow(dead_code)]
const ADD_SP_IMM_BASE: u32 = 0x9100_0000;
#[allow(dead_code)]
const SUB_SP_IMM_BASE: u32 = 0xd100_0000;
const SUBS_W_BASE: u32 = 0x6b00_0000;
const SUBS_X_BASE: u32 = 0xeb00_0000;
const MOVZ_X_BASE: u32 = 0xd280_0000;
const MOVK_X_BASE: u32 = 0xf280_0000;
const ADR_X_BASE: u32 = 0x1000_0000;
#[allow(dead_code)]
const STR_W_SP_UNSIGNED_BASE: u32 = 0xb900_0000;
#[allow(dead_code)]
const LDR_W_SP_UNSIGNED_BASE: u32 = 0xb940_0000;
#[allow(dead_code)]
const STR_X_SP_UNSIGNED_BASE: u32 = 0xf900_0000;
#[allow(dead_code)]
const LDR_X_SP_UNSIGNED_BASE: u32 = 0xf940_0000;
#[allow(dead_code)]
const STR_W_UNSIGNED_BASE: u32 = 0xb900_0000;
const LDR_W_UNSIGNED_BASE: u32 = 0xb940_0000;
const STRB_W_UNSIGNED_BASE: u32 = 0x3900_0000;
const LDRB_W_UNSIGNED_BASE: u32 = 0x3940_0000;
const STR_X_UNSIGNED_BASE: u32 = 0xf900_0000;
const LDR_X_UNSIGNED_BASE: u32 = 0xf940_0000;
const LDRB_W_REG_BASE: u32 = 0x3860_6800;
const B_BASE: u32 = 0x1400_0000;
const B_COND_BASE: u32 = 0x5400_0000;
const BL_BASE: u32 = 0x9400_0000;
const RET_X30: u32 = 0xd65f_03c0;
const SVC_BASE: u32 = 0xd400_0001;
const BRK_BASE: u32 = 0xd420_0000;

const ADR_MIN_BYTE_OFFSET: i32 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i32 = (1 << 20) - 1;
const BL_MIN_BYTE_OFFSET: i32 = -(1 << 27);
const BL_MAX_BYTE_OFFSET: i32 = (1 << 27) - 4;
const B_COND_MIN_BYTE_OFFSET: i32 = -(1 << 20);
const B_COND_MAX_BYTE_OFFSET: i32 = (1 << 20) - 4;
#[allow(dead_code)]
const SP_BITS: u32 = 31;
const WZR_BITS: u32 = 31;
const XZR_BITS: u32 = 31;

const fn move_wide_fields(rd: u32, imm16: u16, shift: MoveWideShift) -> u32 {
    (shift.hw() << 21) | ((imm16 as u32) << 5) | rd
}

fn adr_x_word(rd: XReg, byte_offset: i32) -> u32 {
    debug_assert!((ADR_MIN_BYTE_OFFSET..=ADR_MAX_BYTE_OFFSET).contains(&byte_offset));

    let encoded = (byte_offset as u32) & 0x001f_ffff;
    let immlo = encoded & 0x3;
    let immhi = (encoded >> 2) & 0x7ffff;
    ADR_X_BASE | (immlo << 29) | (immhi << 5) | rd.bits()
}

#[allow(dead_code)]
fn add_sub_sp_imm_word(base: u32, byte_count: u32) -> u32 {
    let (shift, imm12) = if byte_count <= 0x0fff {
        (0, byte_count)
    } else {
        debug_assert_eq!(byte_count % 4096, 0);
        (1, byte_count / 4096)
    };
    debug_assert!(imm12 <= 0x0fff);

    base | (shift << 22) | (imm12 << 10) | (SP_BITS << 5) | SP_BITS
}

fn add_x_sp_imm_word(rd: XReg, byte_count: u32) -> u32 {
    let (shift, imm12) = if byte_count <= 0x0fff {
        (0, byte_count)
    } else {
        debug_assert_eq!(byte_count % 4096, 0);
        (1, byte_count / 4096)
    };
    debug_assert!(imm12 <= 0x0fff);

    ADD_SP_IMM_BASE | (shift << 22) | (imm12 << 10) | (SP_BITS << 5) | rd.bits()
}

#[allow(dead_code)]
fn load_store_sp_word(base: u32, rt: u32, byte_offset: u32, access_size: u32) -> u32 {
    load_store_unsigned_word(base, rt, SP_BITS, byte_offset, access_size)
}

fn load_store_unsigned_word(
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

fn bl_word(byte_offset: i32) -> u32 {
    debug_assert!((BL_MIN_BYTE_OFFSET..=BL_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    BL_BASE | (((byte_offset / 4) as u32) & 0x03ff_ffff)
}

fn b_word(byte_offset: i32) -> u32 {
    debug_assert!((BL_MIN_BYTE_OFFSET..=BL_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    B_BASE | (((byte_offset / 4) as u32) & 0x03ff_ffff)
}

fn b_cond_word(condition: BranchCondition, byte_offset: i32) -> u32 {
    debug_assert!((B_COND_MIN_BYTE_OFFSET..=B_COND_MAX_BYTE_OFFSET).contains(&byte_offset));
    debug_assert_eq!(byte_offset % 4, 0);

    B_COND_BASE | ((((byte_offset / 4) as u32) & 0x0007_ffff) << 5) | condition.bits()
}

#[cfg(test)]
mod tests {
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
    fn encodes_add_w0_w0_w1() {
        let mut encoder = Encoder::new();

        encoder.emit_add_w(WReg::W0, WReg::W0, WReg::W1);

        assert_eq!(encoder.finish(), vec![0x00, 0x00, 0x01, 0x0b]);
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
    fn encodes_ldrb_w0_x1_x2() {
        let mut encoder = Encoder::new();

        encoder.emit_ldrb_w_reg(WReg::W0, XReg::X1, XReg::X2);

        assert_eq!(encoder.finish(), vec![0x20, 0x68, 0x62, 0x38]);
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
}
