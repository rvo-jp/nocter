/// One encodable ARM64 general-purpose register. Register 31 is represented separately as either
/// the zero register or stack pointer so instruction construction cannot confuse their roles.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arm64Register(u8);

impl Arm64Register {
    #[must_use]
    pub const fn new(number: u8) -> Option<Self> {
        if number < 31 {
            Some(Self(number))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

/// A data operand for instructions where encoding 31 means the zero register.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64DataRegister {
    General(Arm64Register),
    Zero,
}

impl Arm64DataRegister {
    pub(crate) const fn encoding(self) -> u32 {
        match self {
            Self::General(register) => register.number() as u32,
            Self::Zero => 31,
        }
    }
}

/// An address or arithmetic-base operand for instructions where encoding 31 means `sp`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64BaseRegister {
    General(Arm64Register),
    StackPointer,
}

impl Arm64BaseRegister {
    pub(crate) const fn encoding(self) -> u32 {
        match self {
            Self::General(register) => register.number() as u32,
            Self::StackPointer => 31,
        }
    }
}

/// Destination of add/subtract-immediate instructions, where encoding 31 changes meaning when
/// condition flags are requested.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64AddSubtractDestination {
    General(Arm64Register),
    StackPointer,
    Zero,
}

impl Arm64AddSubtractDestination {
    pub(crate) const fn encoding(self) -> u32 {
        match self {
            Self::General(register) => register.number() as u32,
            Self::StackPointer | Self::Zero => 31,
        }
    }
}
