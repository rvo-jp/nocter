use std::error::Error;
use std::fmt;

use nocter_source::SourceFile;
use nocter_syntax::{ParseGoal, SyntaxTree, parse};

/// Infrastructure failure returned by a source-syntax provider.
#[derive(Debug)]
pub struct SourceSyntaxError(Box<dyn Error + Send + Sync>);

impl SourceSyntaxError {
    pub fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }
}

impl fmt::Display for SourceSyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Error for SourceSyntaxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// Supplies the lossless syntax product for one already-ingested source.
///
/// Discovery owns when a source is needed and validates the returned tree's identity. A provider
/// may parse directly or bind a reusable source-text product; it cannot influence module topology.
pub trait SourceSyntaxProvider {
    /// Returns syntax bound to `source` and `goal`.
    ///
    /// # Errors
    ///
    /// Returns an infrastructure failure. Authored lexical and parse errors remain ordinary
    /// diagnostics in the returned syntax tree.
    fn syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<SyntaxTree, SourceSyntaxError>;
}

/// Non-caching source parser used outside a revisioned computation owner.
#[derive(Default)]
pub struct DirectSourceSyntax;

impl SourceSyntaxProvider for DirectSourceSyntax {
    fn syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<SyntaxTree, SourceSyntaxError> {
        Ok(parse(source, goal))
    }
}
