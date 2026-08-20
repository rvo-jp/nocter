use crate::Arm64NocterAbi;

/// Compiler-owned physical representation of one non-movable lexical allocation context.
///
/// The first two words are the ordinary hidden-context header consumed through `x9`. The state
/// word points at the third word, which anchors the intrusive list of independently mapped region
/// allocations. The last two words retain the selected parent header used to derive this child.
/// The source-level `AllocationContext` remains opaque and contributes no physical layout
/// authority.
pub(crate) struct Arm64RegionLayout;

impl Arm64RegionLayout {
    pub(crate) const STATE_OFFSET: u64 = 0;
    pub(crate) const KIND_OFFSET: u64 = Arm64NocterAbi::WORD_SIZE;
    pub(crate) const HEAD_OFFSET: u64 = 2 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const PARENT_STATE_OFFSET: u64 = 3 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const PARENT_KIND_OFFSET: u64 = 4 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const SIZE: u64 = 5 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const ALIGNMENT: u64 = Arm64NocterAbi::WORD_SIZE;
    pub(crate) const ALLOCATOR_KIND: u64 = 1;
}
