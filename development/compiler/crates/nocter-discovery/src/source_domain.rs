use std::collections::BTreeMap;
use std::fmt;

use nocter_source::SourceId;

use crate::DiscoveredUnit;

/// One source in the canonical physical order shared by discovery-owned projections.
pub(crate) struct CanonicalSource<'unit> {
    pub(crate) id: SourceId,
    pub(crate) path: &'unit str,
    pub(crate) syntax: usize,
}

pub(crate) fn canonical_sources(
    unit: &DiscoveredUnit,
) -> Result<Vec<CanonicalSource<'_>>, SourceDomainError> {
    let mut sources =
        unit.modules
            .iter()
            .flat_map(|module| module.sources().iter())
            .map(|source| {
                let tree = unit.syntax.get(source.syntax_index()).ok_or_else(|| {
                    SourceDomainError::MissingSyntax(source.canonical_path().into())
                })?;
                Ok(CanonicalSource {
                    id: tree.source(),
                    path: source.canonical_path(),
                    syntax: source.syntax_index(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
    sources.sort_unstable_by_key(|source| source.path);
    let mut ownership = BTreeMap::new();
    let mut paths = BTreeMap::new();
    for source in &sources {
        if ownership.insert(source.id, source.path).is_some() {
            return Err(SourceDomainError::DuplicateSource(source.id));
        }
        if paths.insert(source.path, source.id).is_some() {
            return Err(SourceDomainError::DuplicateSourcePath(source.path.into()));
        }
    }
    Ok(sources)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceDomainError {
    MissingSyntax(Box<str>),
    DuplicateSource(SourceId),
    DuplicateSourcePath(Box<str>),
}

impl fmt::Display for SourceDomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid discovered source domain: {self:?}")
    }
}

impl std::error::Error for SourceDomainError {}
