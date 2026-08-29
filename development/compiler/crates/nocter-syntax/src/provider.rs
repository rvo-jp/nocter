use std::error::Error;
use std::fmt;
use std::sync::Arc;

use nocter_source::SourceFile;

use crate::{ParseGoal, ParsedSyntax, parse_reusable};

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
/// The consumer owns why a source is needed and must validate the returned tree's identity. A
/// provider may parse directly or bind a reusable source-text product; it cannot select sources.
pub trait SourceSyntaxProvider {
    /// Returns a source-text-owned parse product for `source` and `goal`.
    ///
    /// # Errors
    ///
    /// Returns an infrastructure failure. Authored lexical and parse errors remain ordinary
    /// diagnostics in the returned parse product.
    fn parsed_syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<Arc<ParsedSyntax>, SourceSyntaxError>;
}

/// Non-caching source parser used outside a revisioned computation owner.
#[derive(Default)]
pub struct DirectSourceSyntax;

impl SourceSyntaxProvider for DirectSourceSyntax {
    fn parsed_syntax(
        &mut self,
        source: &SourceFile,
        goal: ParseGoal,
    ) -> Result<Arc<ParsedSyntax>, SourceSyntaxError> {
        Ok(Arc::new(parse_reusable(source, goal)))
    }
}
