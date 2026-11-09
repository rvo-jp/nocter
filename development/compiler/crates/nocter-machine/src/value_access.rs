//! Physical activation storage and resource-phase effects. Not an executable `MachineProgram`.
// Initialization recipes encode 64-bit words independently of payload data types.
const FLAG_WORD_BYTES: u64 = 8;
mod lower;

use crate::{MachineAbiError, MachineLayoutError, MachineLayoutStore, MachineValueCallableAbi};
use nocter_mir::value_access as mir;
use nocter_model::{AccessInstanceId, Arena, MirLocalId, TypeId};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlotLayout {
    pub offset: u64,
    pub size: u64,
    pub alignment: u64,
    pub storage: mir::Storage,
}

#[derive(Clone, Debug)]
pub struct Frame {
    slots: Arena<MirLocalId, SlotLayout>,
    size: u64,
    alignment: u64,
    flags_offset: Option<u64>,
    initial_flags: Box<[u64]>,
}
impl Frame {
    #[must_use]
    pub const fn slots(&self) -> &Arena<MirLocalId, SlotLayout> {
        &self.slots
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
    pub const fn flags_offset(&self) -> Option<u64> {
        self.flags_offset
    }
    /// Copy this template into the frame of each new invocation.
    #[must_use]
    pub fn initial_flags(&self) -> &[u64] {
        &self.initial_flags
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressBase {
    Frame,
    Referent { pointer_offset: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Address {
    pub identity: crate::MachineAddressId,
    /// Logical slot identity survives equal zero-byte addresses.
    pub slot: MirLocalId,
    pub base: AddressBase,
    pub offset: u64,
    pub data: TypeId,
    pub size: u64,
    pub alignment: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    Initialize(Address),
    /// A selected transfer or representation adapter; no allocation or cleanup is implicit.
    CopyBytes {
        source: Address,
        destination: Address,
        bytes: u64,
    },
    Dispose {
        source: Address,
        disposal: mir::Disposal,
    },
    End(Address),
    BindAddress {
        destination: u64,
        source: Address,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BitValue {
    pub offset: u64,
    pub mask: u64,
    pub value: bool,
}

#[derive(Clone, Debug)]
pub struct Alternative {
    pub tests: Box<[BitValue]>,
    pub before: Box<[Effect]>,
    pub before_writes: Box<[BitValue]>,
    pub after: Box<[Effect]>,
    pub after_writes: Box<[BitValue]>,
}
#[derive(Clone, Debug)]
pub struct Action {
    pub call_bindings: Box<[Effect]>,
    pub alternatives: Box<[Alternative]>,
}
#[derive(Clone, Debug)]
pub struct Body {
    frame: Frame,
    abi: MachineValueCallableAbi,
    actions: BTreeMap<mir::Location, Action>,
}
impl Body {
    #[must_use]
    pub const fn frame(&self) -> &Frame {
        &self.frame
    }
    #[must_use]
    pub const fn abi(&self) -> &MachineValueCallableAbi {
        &self.abi
    }
    #[must_use]
    pub const fn actions(&self) -> &BTreeMap<mir::Location, Action> {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    Layout(MachineLayoutError),
    Abi(MachineAbiError),
    InvalidBody(AccessInstanceId),
    FrameOverflow(AccessInstanceId),
}

/// A source-independent storage companion with no retained checking or target-program storage.
#[derive(Debug)]
pub struct StorageProgram {
    layouts: MachineLayoutStore,
    bodies: BTreeMap<AccessInstanceId, Body>,
}
impl StorageProgram {
    /// Place each activation and materialize all frozen resource addresses and byte counts.
    /// # Errors
    /// Rejects invalid representation/projection/flag references and overflowing layouts.
    pub fn lower(program: &mir::Program) -> Result<Self, Failure> {
        lower::program(program)
    }
    #[must_use]
    pub const fn layouts(&self) -> &MachineLayoutStore {
        &self.layouts
    }
    #[must_use]
    pub const fn bodies(&self) -> &BTreeMap<AccessInstanceId, Body> {
        &self.bodies
    }
}
