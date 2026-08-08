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
    W8,
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
    pub(crate) const fn to_x(self) -> XReg {
        match self {
            Self::W0 => XReg::X0,
            Self::W1 => XReg::X1,
            Self::W2 => XReg::X2,
            Self::W3 => XReg::X3,
            Self::W4 => XReg::X4,
            Self::W5 => XReg::X5,
            Self::W6 => XReg::X6,
            Self::W7 => XReg::X7,
            Self::W8 => XReg::X8,
            Self::W9 => XReg::X9,
            Self::W10 => XReg::X10,
            Self::W11 => XReg::X11,
            Self::W12 => XReg::X12,
            Self::W13 => XReg::X13,
            Self::W14 => XReg::X14,
            Self::W15 => XReg::X15,
            Self::W16 => XReg::X16,
            Self::W17 => XReg::X17,
        }
    }

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

    pub(in crate::target::arm64::encoder) const fn bits(self) -> u32 {
        match self {
            Self::W0 => 0,
            Self::W1 => 1,
            Self::W2 => 2,
            Self::W3 => 3,
            Self::W4 => 4,
            Self::W5 => 5,
            Self::W6 => 6,
            Self::W7 => 7,
            Self::W8 => 8,
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
    pub(in crate::target::arm64::encoder) const fn bits(self) -> u32 {
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
    X19,
    X20,
    X21,
    X22,
    X23,
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

    pub(in crate::target::arm64::encoder) const fn bits(self) -> u32 {
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
            Self::X19 => 19,
            Self::X20 => 20,
            Self::X21 => 21,
            Self::X22 => 22,
            Self::X23 => 23,
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
    pub(in crate::target::arm64::encoder) const fn hw(self) -> u32 {
        match self {
            Self::Lsl0 => 0,
            Self::Lsl16 => 1,
            Self::Lsl32 => 2,
            Self::Lsl48 => 3,
        }
    }

    pub(in crate::target::arm64::encoder) const fn is_valid_for_wide_32(self) -> bool {
        matches!(self, Self::Lsl0 | Self::Lsl16)
    }
}
