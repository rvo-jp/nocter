//! Normalized source storage and source-coordinate identities.

mod line_index;
mod map;
mod span;

pub use line_index::{LineColumn, LineIndex};
pub use map::{SourceError, SourceFile, SourceMap, SourceName};
pub use span::{ByteOffset, SourceId, Span, TextRange};
