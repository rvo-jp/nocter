use std::fmt;

use nocter_source::SourceId;

use crate::DiscoveredUnit;
use crate::source_domain::{SourceDomainError, canonical_sources};

/// Canonical exact current-source product used only by generation-local query dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentSourceSurface {
    canonical: Box<[u8]>,
}

impl CurrentSourceSurface {
    #[must_use]
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
}

impl DiscoveredUnit {
    /// Freezes every reached source byte in canonical physical-source order.
    ///
    /// # Errors
    ///
    /// Returns an integrity error when physical ownership, syntax storage, and source storage do
    /// not describe the same closed source domain.
    pub fn current_source_surface(
        &self,
    ) -> Result<CurrentSourceSurface, CurrentSourceSurfaceError> {
        let sources = canonical_sources(self)?;
        let mut canonical = Vec::new();
        for source in sources {
            let file = self
                .sources
                .get(source.id)
                .ok_or(CurrentSourceSurfaceError::MissingSource(source.id))?;
            encode(source.path.as_bytes(), &mut canonical);
            encode(file.text().as_bytes(), &mut canonical);
        }
        Ok(CurrentSourceSurface {
            canonical: canonical.into_boxed_slice(),
        })
    }
}

fn encode(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CurrentSourceSurfaceError {
    SourceDomain(SourceDomainError),
    MissingSource(SourceId),
}

impl fmt::Display for CurrentSourceSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid discovered current-source surface: {self:?}"
        )
    }
}

impl std::error::Error for CurrentSourceSurfaceError {}

impl From<SourceDomainError> for CurrentSourceSurfaceError {
    fn from(error: SourceDomainError) -> Self {
        Self::SourceDomain(error)
    }
}
