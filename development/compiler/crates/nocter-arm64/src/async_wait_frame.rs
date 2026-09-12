use crate::{Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObjectId, Arm64NocterAbi};

/// Fixed process-root state retained while native event calls may clobber argument registers.
///
/// The pending interest slice remains owned by the suspended computation. The process root owns
/// one temporary Darwin change/event mapping and one queue descriptor through the complete wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arm64AsyncWaitFrame {
    object: Arm64FrameObjectId,
}

impl Arm64AsyncWaitFrame {
    pub(crate) const INTEREST_POINTER_OFFSET: u64 = 0;
    pub(crate) const INTEREST_COUNT_OFFSET: u64 = 8;
    pub(crate) const MAPPING_POINTER_OFFSET: u64 = 16;
    pub(crate) const MAPPING_SIZE_OFFSET: u64 = 24;
    pub(crate) const EARLIEST_DEADLINE_OFFSET: u64 = 32;
    pub(crate) const NATIVE_COUNT_OFFSET: u64 = 40;
    pub(crate) const EVENT_COUNT_OFFSET: u64 = 48;
    pub(crate) const QUEUE_DESCRIPTOR_OFFSET: u64 = 56;
    pub(crate) const IMMEDIATE_READY_OFFSET: u64 = 64;
    const SIZE: u64 = 72;
    const ALIGNMENT: u64 = 8;

    pub(crate) fn place(
        builder: &mut Arm64FrameLayoutBuilder,
    ) -> Result<Self, Arm64FrameLayoutError> {
        debug_assert_eq!(Self::ALIGNMENT, Arm64NocterAbi::word_size());
        Ok(Self {
            object: builder.add_object(Self::SIZE, Self::ALIGNMENT)?,
        })
    }

    #[must_use]
    pub const fn object(self) -> Arm64FrameObjectId {
        self.object
    }
}
