#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn emit_movz_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVZ_W_BASE | move_wide_fields(rd, imm16, shift));
    }

    pub(crate) fn emit_movk_w(&mut self, rd: WReg, imm16: u16, shift: MoveWideShift) {
        self.emit_word(MOVK_W_BASE | move_wide_fields(rd, imm16, shift));
    }

    pub(crate) fn emit_svc(&mut self, imm16: u16) {
        self.emit_word(SVC_BASE | ((imm16 as u32) << 5));
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
pub(crate) enum MoveWideShift {
    Lsl0,
    Lsl16,
}

impl MoveWideShift {
    const fn hw(self) -> u32 {
        match self {
            Self::Lsl0 => 0,
            Self::Lsl16 => 1,
        }
    }
}

const MOVZ_W_BASE: u32 = 0x5280_0000;
const MOVK_W_BASE: u32 = 0x7280_0000;
const SVC_BASE: u32 = 0xd400_0001;

const fn move_wide_fields(rd: WReg, imm16: u16, shift: MoveWideShift) -> u32 {
    (shift.hw() << 21) | ((imm16 as u32) << 5) | rd.bits()
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
    fn encodes_svc_imm16() {
        let mut encoder = Encoder::new();

        encoder.emit_svc(0x80);

        assert_eq!(encoder.finish(), vec![0x01, 0x10, 0x00, 0xd4]);
    }
}
