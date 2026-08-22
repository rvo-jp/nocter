use nocter_source::{SourceId, TextRange};

/// One protocol-independent source replacement selected by compiler analysis.
///
/// The source identity and replacement text travel together so mutation features cannot silently
/// project an edit onto the document that happened to receive the request.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticSourceEdit {
    source: SourceId,
    range: TextRange,
    new_text: Box<str>,
}

impl SemanticSourceEdit {
    #[must_use]
    pub fn new(source: SourceId, range: TextRange, new_text: impl Into<Box<str>>) -> Self {
        Self {
            source,
            range,
            new_text: new_text.into(),
        }
    }

    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new_text
    }
}
