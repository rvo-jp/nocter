use std::collections::BTreeSet;
use std::fmt;

use crate::{Arm64NocterAbi, Arm64Register};

const STACK_ALIGNMENT: u64 = Arm64NocterAbi::STACK_ALIGNMENT;
const REGISTER_SIZE: u64 = Arm64NocterAbi::WORD_SIZE;
const FRAME_RECORD_SIZE: u64 = 2 * REGISTER_SIZE;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Arm64FrameObjectId(usize);

/// One caller-, selector-, or allocator-owned byte range in a fixed ARM64 frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64FrameObject {
    offset: u64,
    size: u64,
    alignment: u64,
}

impl Arm64FrameObject {
    /// Byte offset from the post-prologue stack pointer.
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }

    #[must_use]
    pub const fn size(self) -> u64 {
        self.size
    }

    #[must_use]
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// One preserved general register and its eight-byte frame slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64SavedRegister {
    register: Arm64Register,
    offset: u64,
}

impl Arm64SavedRegister {
    #[must_use]
    pub const fn register(self) -> Arm64Register {
        self.register
    }

    /// Byte offset from the post-prologue stack pointer.
    #[must_use]
    pub const fn offset(self) -> u64 {
        self.offset
    }
}

/// Complete fixed-frame placement before prologue and epilogue instruction selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64FrameLayout {
    size: u64,
    outgoing_argument_size: u64,
    objects: Box<[Arm64FrameObject]>,
    saved_registers: Box<[Arm64SavedRegister]>,
    frame_record_offset: u64,
}

impl Arm64FrameLayout {
    /// Total stack-pointer decrement. This is always a multiple of the call-boundary alignment.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Reserved range beginning at the post-prologue stack pointer for stack-passed arguments.
    #[must_use]
    pub const fn outgoing_argument_size(&self) -> u64 {
        self.outgoing_argument_size
    }

    #[must_use]
    pub fn object(&self, id: Arm64FrameObjectId) -> Option<Arm64FrameObject> {
        self.objects.get(id.0).copied()
    }

    #[must_use]
    pub const fn saved_registers(&self) -> &[Arm64SavedRegister] {
        &self.saved_registers
    }

    /// Slot containing the previous `x29`, followed by the saved `x30` link register.
    #[must_use]
    pub const fn frame_record_offset(&self) -> u64 {
        self.frame_record_offset
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrameObjectRequest {
    size: u64,
    alignment: u64,
}

/// Deterministic allocator for one function's fixed ARM64 stack frame.
#[derive(Default)]
pub struct Arm64FrameLayoutBuilder {
    outgoing_argument_size: u64,
    objects: Vec<FrameObjectRequest>,
    saved_registers: BTreeSet<Arm64Register>,
}

impl Arm64FrameLayoutBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            outgoing_argument_size: 0,
            objects: Vec::new(),
            saved_registers: BTreeSet::new(),
        }
    }

    /// Reserves the maximum stack-argument area required by any call in the function.
    ///
    /// # Errors
    ///
    /// Rejects a size that would violate ARM64 call-boundary stack alignment.
    pub fn require_outgoing_argument_size(
        &mut self,
        size: u64,
    ) -> Result<(), Arm64FrameLayoutError> {
        if !size.is_multiple_of(STACK_ALIGNMENT) {
            return Err(Arm64FrameLayoutError::MisalignedOutgoingArguments(size));
        }
        self.outgoing_argument_size = self.outgoing_argument_size.max(size);
        Ok(())
    }

    /// Adds a frame object in stable insertion order.
    ///
    /// # Errors
    ///
    /// Rejects non-power-of-two alignment or alignment greater than the guaranteed stack
    /// alignment. Current Nocter stored layouts do not exceed that target limit.
    pub fn add_object(
        &mut self,
        size: u64,
        alignment: u64,
    ) -> Result<Arm64FrameObjectId, Arm64FrameLayoutError> {
        validate_alignment(alignment)?;
        let id = Arm64FrameObjectId(self.objects.len());
        self.objects.push(FrameObjectRequest { size, alignment });
        Ok(id)
    }

    /// Requests preservation of one ABI callee-saved register. Repeated requests are idempotent.
    ///
    /// # Errors
    ///
    /// Rejects caller-saved, reserved, frame-pointer, and link registers.
    pub fn preserve(&mut self, register: Arm64Register) -> Result<(), Arm64FrameLayoutError> {
        if !Arm64NocterAbi::is_callee_saved(register) {
            return Err(Arm64FrameLayoutError::NotCalleeSaved(register));
        }
        self.saved_registers.insert(register);
        Ok(())
    }

    /// Places every range and the canonical `x29`/`x30` frame record.
    ///
    /// # Errors
    ///
    /// Rejects offset or total-frame arithmetic overflow.
    pub fn finish(self) -> Result<Arm64FrameLayout, Arm64FrameLayoutError> {
        let mut next = self.outgoing_argument_size;
        let mut objects = Vec::with_capacity(self.objects.len());
        for request in self.objects {
            let offset = align_up(next, request.alignment)?;
            next = offset
                .checked_add(request.size)
                .ok_or(Arm64FrameLayoutError::FrameOverflow)?;
            objects.push(Arm64FrameObject {
                offset,
                size: request.size,
                alignment: request.alignment,
            });
        }

        let mut saved_registers = Vec::with_capacity(self.saved_registers.len());
        for register in self.saved_registers {
            let offset = align_up(next, REGISTER_SIZE)?;
            next = offset
                .checked_add(REGISTER_SIZE)
                .ok_or(Arm64FrameLayoutError::FrameOverflow)?;
            saved_registers.push(Arm64SavedRegister { register, offset });
        }

        let frame_size = align_up(
            next.checked_add(FRAME_RECORD_SIZE)
                .ok_or(Arm64FrameLayoutError::FrameOverflow)?,
            STACK_ALIGNMENT,
        )?;
        let frame_record_offset = frame_size - FRAME_RECORD_SIZE;
        Ok(Arm64FrameLayout {
            size: frame_size,
            outgoing_argument_size: self.outgoing_argument_size,
            objects: objects.into_boxed_slice(),
            saved_registers: saved_registers.into_boxed_slice(),
            frame_record_offset,
        })
    }
}

fn validate_alignment(alignment: u64) -> Result<(), Arm64FrameLayoutError> {
    if !alignment.is_power_of_two() || alignment > STACK_ALIGNMENT {
        return Err(Arm64FrameLayoutError::InvalidObjectAlignment(alignment));
    }
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Arm64FrameLayoutError> {
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(Arm64FrameLayoutError::FrameOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64FrameLayoutError {
    MisalignedOutgoingArguments(u64),
    InvalidObjectAlignment(u64),
    NotCalleeSaved(Arm64Register),
    FrameOverflow,
}

impl fmt::Display for Arm64FrameLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 frame layout failed: {self:?}")
    }
}

impl std::error::Error for Arm64FrameLayoutError {}
