use nocter_model::TypeId;

/// Why one abstract stack object exists before physical frame placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineStackPurpose {
    Parameter { position: usize },
    User,
    Temporary,
    Region,
}

/// One body-local object with frozen size and alignment but no physical frame offset yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineStackObject {
    ty: TypeId,
    size: u64,
    alignment: u64,
    purpose: MachineStackPurpose,
}

/// One body-local conditional-initialization bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineDropFlag {
    initially_initialized: bool,
}

impl MachineDropFlag {
    pub(crate) const fn new(initially_initialized: bool) -> Self {
        Self {
            initially_initialized,
        }
    }

    #[must_use]
    pub const fn initially_initialized(self) -> bool {
        self.initially_initialized
    }
}

impl MachineStackObject {
    pub(crate) const fn new(
        ty: TypeId,
        size: u64,
        alignment: u64,
        purpose: MachineStackPurpose,
    ) -> Self {
        Self {
            ty,
            size,
            alignment,
            purpose,
        }
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn purpose(self) -> MachineStackPurpose {
        self.purpose
    }
}
