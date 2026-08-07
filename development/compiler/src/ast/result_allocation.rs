use crate::source::ByteSpan;

/// The source-level `alloc` contract attached to a callable declaration or type.
///
/// This remains distinct from execution-time allocation requirements: it says
/// that newly allocated storage may be retained in the returned value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultAllocationModifier {
    pub span: ByteSpan,
}
