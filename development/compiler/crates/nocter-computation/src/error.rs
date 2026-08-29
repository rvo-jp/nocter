use std::fmt;

use crate::ComputationIdentity;

/// Failure of the computation kernel rather than a compiler-domain outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ComputationError {
    MissingInput(ComputationIdentity),
    Cycle(Box<[ComputationIdentity]>),
    StoredTypeMismatch(ComputationIdentity),
    RevisionExhausted,
}

impl fmt::Display for ComputationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput(identity) => {
                write!(formatter, "computation input {} is absent", identity.name())
            }
            Self::Cycle(cycle) => {
                formatter.write_str("computation query cycle")?;
                for identity in cycle {
                    write!(formatter, " -> {}", identity.name())?;
                }
                Ok(())
            }
            Self::StoredTypeMismatch(identity) => write!(
                formatter,
                "stored computation value has the wrong type for {}",
                identity.name()
            ),
            Self::RevisionExhausted => {
                formatter.write_str("computation revision identity space is exhausted")
            }
        }
    }
}

impl std::error::Error for ComputationError {}
