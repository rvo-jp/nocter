use crate::{Arm64FrameLayoutBuilder, Arm64FrameLayoutError, Arm64FrameObjectId, Arm64NocterAbi};

/// Fixed process-root state retained while a native wait call may clobber argument registers.
///
/// The pending interest slice remains owned by the suspended computation. The mapping is a
/// temporary Darwin `pollfd` array owned by the process root and released before resumption.
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
    const SIZE: u64 = 40;
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
