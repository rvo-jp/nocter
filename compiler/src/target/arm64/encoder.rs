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

    pub(crate) fn emit_movz_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVZ_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_movk_x(&mut self, rd: XReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVK_X_BASE | move_wide_fields(rd.bits(), imm16, shift));
    }

    pub(crate) fn emit_adr_x(&mut self, rd: XReg, byte_offset: i32) {
        self.emit_word(adr_x_word(rd, byte_offset));
    }

    pub(crate) fn emit_b(&mut self, byte_offset: i32) {
        self.emit_word(b_word(byte_offset));
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
    W16,
}

impl WReg {
    const fn bits(self) -> u32 {
        match self {
            Self::W0 => 0,
            Self::W16 => 16,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum XReg {
    X0,
    X1,
    X2,
}

impl XReg {
    const fn bits(self) -> u32 {
        match self {
            Self::X0 => 0,
            Self::X1 => 1,
            Self::X2 => 2,
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
const MOVZ_X_BASE: u32 = 0xd280_0000;
const MOVK_X_BASE: u32 = 0xf280_0000;
const ADR_X_BASE: u32 = 0x1000_0000;
const B_BASE: u32 = 0x1400_0000;
const BL_BASE: u32 = 0x9400_0000;
const RET_X30: u32 = 0xd65f_03c0;
const SVC_BASE: u32 = 0xd400_0001;

const ADR_MIN_BYTE_OFFSET: i32 = -(1 << 20);
const ADR_MAX_BYTE_OFFSET: i32 = (1 << 20) - 1;
const BL_MIN_BYTE_OFFSET: i32 = -(1 << 27);
const BL_MAX_BYTE_OFFSET: i32 = (1 << 27) - 4;

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
}
