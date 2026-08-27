use std::fmt;

use nocter_source::SourceId;

/// An incomplete source ownership fact at the protocol-independent query boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContextError {
    MissingModuleOwner(SourceId),
}

impl fmt::Display for SourceContextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingModuleOwner(source) => {
                write!(formatter, "source {source} has no semantic module owner")
            }
        }
    }
}

impl std::error::Error for SourceContextError {}
