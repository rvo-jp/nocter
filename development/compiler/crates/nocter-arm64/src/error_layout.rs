use crate::Arm64NocterAbi;

/// Target-owned offsets for the compiler built-in error payload and its allocation-free report
/// scratch. The machine layout is validated against these offsets before selection.
pub(crate) struct Arm64ErrorLayout;

impl Arm64ErrorLayout {
    pub(crate) const SIZE: u64 = 4 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const ALIGNMENT: u64 = Arm64NocterAbi::WORD_SIZE;
    pub(crate) const CODE_OFFSET: u64 = 0;
    pub(crate) const MESSAGE_OFFSET: u64 = 2 * Arm64NocterAbi::WORD_SIZE;
    pub(crate) const VIEW_POINTER_OFFSET: u64 = 0;
    pub(crate) const VIEW_LENGTH_OFFSET: u64 = Arm64NocterAbi::WORD_SIZE;

    // Eight bytes let materialization initialize `": \n"` with one bounded word store while
    // exposing only its first three bytes to write(2).
    pub(crate) const REPORT_BUFFER_SIZE: u64 = Arm64NocterAbi::WORD_SIZE;
    pub(crate) const REPORT_BUFFER_ALIGNMENT: u64 = Arm64NocterAbi::WORD_SIZE;
}
