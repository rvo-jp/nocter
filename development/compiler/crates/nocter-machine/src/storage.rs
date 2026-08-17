use nocter_model::TypeId;

use crate::{MachineStackId, MachineValueId};

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

/// The base from which one checked machine address is evaluated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAddressRoot {
    Stack(MachineStackId),
    Pointer {
        value: MachineValueId,
    },
    View {
        value: MachineValueId,
        pointer_offset: u64,
        length_offset: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineIndex {
    Constant(u64),
    Value(MachineValueId),
}

/// Runtime bound used by an address index step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineIndexBound {
    Fixed(u64),
    /// Length retained by the nearest preceding view root or view-dereference step.
    CurrentView,
}

/// One explicit target-independent address calculation after semantic projections are erased.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineAddressStep {
    Offset(u64),
    Dereference,
    ViewDereference {
        pointer_offset: u64,
        length_offset: u64,
    },
    Index {
        index: MachineIndex,
        stride: u64,
        bound: MachineIndexBound,
    },
}

/// One interned checked path to stored bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineAddress {
    ty: TypeId,
    size: u64,
    alignment: u64,
    root: MachineAddressRoot,
    steps: Box<[MachineAddressStep]>,
}

impl MachineAddress {
    pub(crate) fn new(
        ty: TypeId,
        size: u64,
        alignment: u64,
        root: MachineAddressRoot,
        steps: impl Into<Box<[MachineAddressStep]>>,
    ) -> Self {
        Self {
            ty,
            size,
            alignment,
            root,
            steps: steps.into(),
        }
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    #[must_use]
    pub const fn root(&self) -> MachineAddressRoot {
        self.root
    }

    #[must_use]
    pub const fn steps(&self) -> &[MachineAddressStep] {
        &self.steps
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
