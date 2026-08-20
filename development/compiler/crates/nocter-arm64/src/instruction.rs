use crate::{
    Arm64AddSubtractDestination, Arm64BaseRegister, Arm64DataRegister, Arm64EncodingError,
    Arm64Register,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DataSize {
    Bits32,
    Bits64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64LoadStoreSize {
    Byte,
    Half,
    Word,
    Double,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AddSubtract {
    Add,
    Subtract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64Logical {
    And,
    Or,
    ExclusiveOr,
    AndSetFlags,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64MoveWide {
    Zero,
    Keep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64Shift {
    Left,
    RightLogical,
    RightArithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64BranchCondition {
    Equal,
    NotEqual,
    CarrySet,
    CarryClear,
    Minus,
    Plus,
    Overflow,
    NoOverflow,
    UnsignedHigher,
    UnsignedLowerOrSame,
    SignedGreaterOrEqual,
    SignedLess,
    SignedGreater,
    SignedLessOrEqual,
}

impl Arm64BranchCondition {
    #[must_use]
    pub const fn invert(self) -> Self {
        match self {
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::CarrySet => Self::CarryClear,
            Self::CarryClear => Self::CarrySet,
            Self::Minus => Self::Plus,
            Self::Plus => Self::Minus,
            Self::Overflow => Self::NoOverflow,
            Self::NoOverflow => Self::Overflow,
            Self::UnsignedHigher => Self::UnsignedLowerOrSame,
            Self::UnsignedLowerOrSame => Self::UnsignedHigher,
            Self::SignedGreaterOrEqual => Self::SignedLess,
            Self::SignedLess => Self::SignedGreaterOrEqual,
            Self::SignedGreater => Self::SignedLessOrEqual,
            Self::SignedLessOrEqual => Self::SignedGreater,
        }
    }

    pub(crate) const fn encoding(self) -> u8 {
        match self {
            Self::Equal => 0,
            Self::NotEqual => 1,
            Self::CarrySet => 2,
            Self::CarryClear => 3,
            Self::Minus => 4,
            Self::Plus => 5,
            Self::Overflow => 6,
            Self::NoOverflow => 7,
            Self::UnsignedHigher => 8,
            Self::UnsignedLowerOrSame => 9,
            Self::SignedGreaterOrEqual => 10,
            Self::SignedLess => 11,
            Self::SignedGreater => 12,
            Self::SignedLessOrEqual => 13,
        }
    }
}

/// Closed instruction subset required by Nocter's integer, memory, call, and control lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64Instruction {
    NoOperation,
    /// Forms the page containing `pc + displacement`. The displacement must be page-aligned.
    AddressPage {
        destination: Arm64Register,
        displacement: i64,
    },
    AddSubtractImmediate {
        size: Arm64DataSize,
        operation: Arm64AddSubtract,
        set_flags: bool,
        destination: Arm64AddSubtractDestination,
        source: Arm64BaseRegister,
        immediate: u16,
        shift_12: bool,
    },
    AddSubtractRegister {
        size: Arm64DataSize,
        operation: Arm64AddSubtract,
        set_flags: bool,
        destination: Arm64DataRegister,
        left: Arm64DataRegister,
        right: Arm64DataRegister,
    },
    /// 64-bit add/subtract with an unsigned-extended 64-bit right operand. This is the form that
    /// permits `sp` as the source or destination during large frame adjustments.
    AddSubtractExtendedRegister {
        operation: Arm64AddSubtract,
        set_flags: bool,
        destination: Arm64AddSubtractDestination,
        left: Arm64BaseRegister,
        right: Arm64Register,
        shift: u8,
    },
    LogicalRegister {
        size: Arm64DataSize,
        operation: Arm64Logical,
        destination: Arm64DataRegister,
        left: Arm64DataRegister,
        right: Arm64DataRegister,
    },
    MoveWide {
        size: Arm64DataSize,
        operation: Arm64MoveWide,
        destination: Arm64Register,
        immediate: u16,
        shift: u8,
    },
    MultiplyAdd {
        size: Arm64DataSize,
        destination: Arm64Register,
        left: Arm64Register,
        right: Arm64Register,
        addend: Arm64DataRegister,
        subtract_product: bool,
    },
    Divide {
        size: Arm64DataSize,
        destination: Arm64Register,
        left: Arm64Register,
        right: Arm64Register,
        signed: bool,
    },
    VariableShift {
        size: Arm64DataSize,
        operation: Arm64Shift,
        destination: Arm64Register,
        value: Arm64Register,
        amount: Arm64Register,
    },
    LoadUnsigned {
        size: Arm64LoadStoreSize,
        destination: Arm64DataRegister,
        base: Arm64BaseRegister,
        offset: u32,
    },
    /// Loads a signed byte, halfword, or word and sign-extends it to the selected register width.
    LoadSigned {
        size: Arm64LoadStoreSize,
        destination_size: Arm64DataSize,
        destination: Arm64DataRegister,
        base: Arm64BaseRegister,
        offset: u32,
    },
    StoreUnsigned {
        size: Arm64LoadStoreSize,
        source: Arm64DataRegister,
        base: Arm64BaseRegister,
        offset: u32,
    },
    ConditionalSet {
        size: Arm64DataSize,
        destination: Arm64Register,
        condition: Arm64BranchCondition,
    },
    Branch {
        displacement: i64,
        link: bool,
    },
    BranchConditional {
        displacement: i64,
        condition: Arm64BranchCondition,
    },
    BranchRegister {
        target: Arm64Register,
        link: bool,
    },
    Return {
        target: Arm64Register,
    },
    Break {
        immediate: u16,
    },
    SupervisorCall {
        immediate: u16,
    },
}

impl Arm64Instruction {
    /// Encodes one instruction as the target's little-endian four-byte word.
    ///
    /// # Errors
    ///
    /// Rejects immediate, shift, scaled offset, or branch displacement values that the selected
    /// instruction form cannot represent.
    pub fn encode(self) -> Result<[u8; 4], Arm64EncodingError> {
        crate::encode::encode(self).map(u32::to_le_bytes)
    }
}
