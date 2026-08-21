use std::fmt;

use nocter_machine::{MachineBody, MachinePack, MachinePackSegment, MachineValueRepresentation};

use crate::Arm64NocterAbi;

const CURSOR_SIZE: u64 = Arm64NocterAbi::WORD_SIZE;

/// Caller-owned descriptor passed through the literal ABI lane.
///
/// The four words are the call-site state pointer, immutable total length, next callback, and
/// residual-destruction callback. Callback code and state layout remain target-owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64PackDescriptorLayout;

impl Arm64PackDescriptorLayout {
    pub const STATE_POINTER_OFFSET: u64 = 0;
    pub const LENGTH_OFFSET: u64 = Arm64NocterAbi::WORD_SIZE;
    pub const NEXT_CALLBACK_OFFSET: u64 = 2 * Arm64NocterAbi::WORD_SIZE;
    pub const DESTROY_CALLBACK_OFFSET: u64 = 3 * Arm64NocterAbi::WORD_SIZE;
    pub const SIZE: u64 = 4 * Arm64NocterAbi::WORD_SIZE;
    pub const ALIGNMENT: u64 = Arm64NocterAbi::WORD_SIZE;
}

/// Bytes owned by one fixed or spread segment in a call-site pack state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64PackSegmentLayout {
    Value {
        value_offset: u64,
        size: u64,
        alignment: u64,
    },
    Spread {
        remaining_offset: u64,
        iterator_offset: u64,
        iterator_size: u64,
        iterator_alignment: u64,
    },
}

/// Complete state-object layout used by callbacks generated for one machine pack identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arm64PackStateLayout {
    cursor_offset: u64,
    size: u64,
    alignment: u64,
    segments: Box<[Arm64PackSegmentLayout]>,
}

impl Arm64PackStateLayout {
    /// Computes one deterministic state layout in source segment order.
    ///
    /// # Errors
    ///
    /// Rejects missing values or addresses, non-stored pack state, invalid alignment, a
    /// non-word remaining count, or offset overflow.
    pub fn build(body: &MachineBody, pack: &MachinePack) -> Result<Self, Arm64PackLayoutError> {
        let mut next = CURSOR_SIZE;
        let mut state_alignment = Arm64NocterAbi::WORD_SIZE;
        let mut segments = Vec::with_capacity(pack.segments().len());
        for segment in pack.segments() {
            let layout = match segment {
                MachinePackSegment::Value { value, .. } => {
                    let (size, alignment) = stored_value(body, *value)?;
                    let value_offset = align_up(next, alignment)?;
                    next = value_offset
                        .checked_add(size)
                        .ok_or(Arm64PackLayoutError::SizeOverflow)?;
                    state_alignment = state_alignment.max(alignment);
                    Arm64PackSegmentLayout::Value {
                        value_offset,
                        size,
                        alignment,
                    }
                }
                MachinePackSegment::Spread(spread) => {
                    let (remaining_size, remaining_alignment) =
                        stored_value(body, spread.remaining())?;
                    if remaining_size != Arm64NocterAbi::WORD_SIZE
                        || remaining_alignment > Arm64NocterAbi::WORD_SIZE
                    {
                        return Err(Arm64PackLayoutError::InvalidRemaining(spread.remaining()));
                    }
                    let remaining_offset = align_up(next, Arm64NocterAbi::WORD_SIZE)?;
                    next = remaining_offset
                        .checked_add(Arm64NocterAbi::WORD_SIZE)
                        .ok_or(Arm64PackLayoutError::SizeOverflow)?;
                    let iterator = body
                        .address(spread.iterator())
                        .ok_or(Arm64PackLayoutError::UnknownAddress(spread.iterator()))?;
                    let iterator_size = iterator
                        .stored_size()
                        .ok_or(Arm64PackLayoutError::NonStoredAddress(spread.iterator()))?;
                    let iterator_alignment = iterator
                        .stored_alignment()
                        .ok_or(Arm64PackLayoutError::NonStoredAddress(spread.iterator()))?;
                    validate_alignment(iterator_alignment)?;
                    let iterator_offset = align_up(next, iterator_alignment)?;
                    next = iterator_offset
                        .checked_add(iterator_size)
                        .ok_or(Arm64PackLayoutError::SizeOverflow)?;
                    state_alignment = state_alignment.max(iterator_alignment);
                    Arm64PackSegmentLayout::Spread {
                        remaining_offset,
                        iterator_offset,
                        iterator_size,
                        iterator_alignment,
                    }
                }
            };
            segments.push(layout);
        }
        let size = align_up(next, state_alignment)?;
        Ok(Self {
            cursor_offset: 0,
            size,
            alignment: state_alignment,
            segments: segments.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn cursor_offset(&self) -> u64 {
        self.cursor_offset
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
    pub const fn segments(&self) -> &[Arm64PackSegmentLayout] {
        &self.segments
    }
}

fn stored_value(
    body: &MachineBody,
    value: nocter_machine::MachineValueId,
) -> Result<(u64, u64), Arm64PackLayoutError> {
    match body
        .value(value)
        .ok_or(Arm64PackLayoutError::UnknownValue(value))?
        .representation()
    {
        MachineValueRepresentation::Stored { size, alignment } => {
            validate_alignment(alignment)?;
            Ok((size, alignment))
        }
        MachineValueRepresentation::Completion | MachineValueRepresentation::Diverging => {
            Err(Arm64PackLayoutError::NonStoredValue(value))
        }
    }
}

fn validate_alignment(alignment: u64) -> Result<(), Arm64PackLayoutError> {
    if !alignment.is_power_of_two() || alignment > Arm64NocterAbi::STACK_ALIGNMENT {
        return Err(Arm64PackLayoutError::InvalidAlignment(alignment));
    }
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Result<u64, Arm64PackLayoutError> {
    validate_alignment(alignment)?;
    let mask = alignment - 1;
    value
        .checked_add(mask)
        .map(|value| value & !mask)
        .ok_or(Arm64PackLayoutError::SizeOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arm64PackLayoutError {
    UnknownValue(nocter_machine::MachineValueId),
    NonStoredValue(nocter_machine::MachineValueId),
    UnknownAddress(nocter_machine::MachineAddressId),
    NonStoredAddress(nocter_machine::MachineAddressId),
    InvalidRemaining(nocter_machine::MachineValueId),
    InvalidAlignment(u64),
    SizeOverflow,
}

impl fmt::Display for Arm64PackLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ARM64 pack layout failed: {self:?}")
    }
}

impl std::error::Error for Arm64PackLayoutError {}
