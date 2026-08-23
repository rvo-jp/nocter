use crate::Arm64NocterAbi;

/// Immutable process-lifetime state captured by a compiler-owned native root.
pub(crate) struct Arm64ProcessContextLayout;

impl Arm64ProcessContextLayout {
    pub(crate) const ARGUMENT_COUNT_OFFSET: u64 = 0;
    pub(crate) const ARGUMENT_VECTOR_OFFSET: u64 = Arm64NocterAbi::word_size();
    pub(crate) const ENVIRONMENT_VECTOR_OFFSET: u64 = 2 * Arm64NocterAbi::word_size();
    pub(crate) const ENVIRONMENT_COUNT_OFFSET: u64 = 3 * Arm64NocterAbi::word_size();
    pub(crate) const SIZE: u64 = 4 * Arm64NocterAbi::word_size();
    pub(crate) const ALIGNMENT: u64 = Arm64NocterAbi::word_size();
}
