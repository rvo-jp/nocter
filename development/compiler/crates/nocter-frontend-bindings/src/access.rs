use std::collections::HashMap;
use std::fmt;

use nocter_model::{DeclarationSiteId, ModuleId, NominalTypeId};
use nocter_source::SourceId;

use crate::FrontendBindingDefinitionError;

/// Closed direct-source visibility selected by declaration lowering.
///
/// This table is semantic input, not an editor projection. A private declaration site is visible
/// only from its authored source and sources that directly see that source.
#[derive(Clone, Debug, Default)]
pub struct SourceAccessTable {
    visible_sources: HashMap<SourceId, Box<[SourceId]>>,
    ownership: SourceOwnershipTable,
    site_sources: HashMap<DeclarationSiteId, SourceId>,
    representations: HashMap<NominalTypeId, NominalRepresentationAccess>,
}

/// Closed physical-source ownership selected by declaration lowering.
///
/// This narrow capability is retained independently by editor recovery stages. Consumers can
/// identify a source's module without inspecting presentation occurrences or acquiring private
/// visibility authority.
#[derive(Clone, Debug, Default)]
pub struct SourceOwnershipTable {
    modules: HashMap<SourceId, ModuleId>,
}

impl SourceOwnershipTable {
    /// Returns the one semantic module that owns a physical source.
    ///
    /// # Errors
    ///
    /// Returns [`SourceOwnershipError`] when lowering did not publish the relation.
    pub fn module_for_source(&self, source: SourceId) -> Result<ModuleId, SourceOwnershipError> {
        self.modules
            .get(&source)
            .copied()
            .ok_or(SourceOwnershipError::MissingSource(source))
    }
}

/// An incomplete source-ownership contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceOwnershipError {
    MissingSource(SourceId),
    ConflictingSource {
        source: SourceId,
        existing: ModuleId,
        duplicate: ModuleId,
    },
}

impl fmt::Display for SourceOwnershipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource(source) => {
                write!(formatter, "source {source} has no semantic module owner")
            }
            Self::ConflictingSource {
                source,
                existing,
                duplicate,
            } => write!(
                formatter,
                "source {source} has conflicting semantic module owners {existing:?} and {duplicate:?}"
            ),
        }
    }
}

impl std::error::Error for SourceOwnershipError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NominalRepresentationAccess {
    source: SourceId,
    contract_private: bool,
}

impl SourceAccessTable {
    /// Returns the one semantic module that owns a physical source.
    ///
    /// # Errors
    ///
    /// Returns an error when lowering did not publish the source-access relation.
    pub fn module_for_source(&self, source: SourceId) -> Result<ModuleId, SourceAccessError> {
        self.ownership
            .modules
            .get(&source)
            .copied()
            .ok_or(SourceAccessError::MissingSourceModule(source))
    }

    /// Returns the narrow source-to-module authority embedded in this access table.
    #[must_use]
    pub const fn ownership(&self) -> &SourceOwnershipTable {
        &self.ownership
    }

    /// Determines whether `from` has direct source access to a private declaration site.
    ///
    /// # Errors
    ///
    /// Returns an error when lowering did not publish either side of the access relation.
    pub fn can_access_private(
        &self,
        from: SourceId,
        site: DeclarationSiteId,
    ) -> Result<bool, SourceAccessError> {
        let visible = self
            .visible_sources
            .get(&from)
            .ok_or(SourceAccessError::MissingSourceVisibility(from))?;
        let declaring_source = self
            .site_sources
            .get(&site)
            .copied()
            .ok_or(SourceAccessError::MissingSite(site))?;
        Ok(visible.binary_search(&declaring_source).is_ok())
    }

    /// Determines whether `from` has direct source access to a nominal representation.
    ///
    /// This remains distinct from access to the nominal's public declaration site: a bodyless
    /// `index.nct` contract and its private field or variant representation have different source
    /// owners even though they share one semantic nominal identity.
    ///
    /// # Errors
    ///
    /// Returns an error when lowering did not publish either side of the access relation.
    pub fn can_access_representation(
        &self,
        from: SourceId,
        nominal: NominalTypeId,
    ) -> Result<bool, SourceAccessError> {
        let visible = self
            .visible_sources
            .get(&from)
            .ok_or(SourceAccessError::MissingSourceVisibility(from))?;
        let declaring_source = self
            .representations
            .get(&nominal)
            .map(|representation| representation.source)
            .ok_or(SourceAccessError::MissingRepresentation(nominal))?;
        Ok(visible.binary_search(&declaring_source).is_ok())
    }

    /// Reports whether a bodyless public nominal contract seals its representation.
    ///
    /// A contract-private representation is not an external structural construction entry even
    /// when it is empty and therefore has no private field site from which to infer that fact.
    ///
    /// # Errors
    ///
    /// Returns an error when lowering did not publish the nominal representation relation.
    pub fn representation_is_contract_private(
        &self,
        nominal: NominalTypeId,
    ) -> Result<bool, SourceAccessError> {
        self.representations
            .get(&nominal)
            .map(|representation| representation.contract_private)
            .ok_or(SourceAccessError::MissingRepresentation(nominal))
    }
}

/// An inconsistent declaration-lowering source-access contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAccessError {
    MissingSourceModule(SourceId),
    MissingSourceVisibility(SourceId),
    MissingSite(DeclarationSiteId),
    MissingRepresentation(NominalTypeId),
}

impl fmt::Display for SourceAccessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceModule(source) => {
                write!(formatter, "missing semantic module for source {source}")
            }
            Self::MissingSourceVisibility(source) => {
                write!(
                    formatter,
                    "missing direct visibility set for source {source}"
                )
            }
            Self::MissingSite(site) => {
                write!(formatter, "missing declaring source for site {site:?}")
            }
            Self::MissingRepresentation(nominal) => {
                write!(formatter, "missing representation source for {nominal:?}")
            }
        }
    }
}

impl std::error::Error for SourceAccessError {}

#[derive(Debug, Default)]
pub(crate) struct SourceAccessTableBuilder {
    visible_sources: HashMap<SourceId, Vec<SourceId>>,
    source_modules: HashMap<SourceId, ModuleId>,
    site_sources: HashMap<DeclarationSiteId, SourceId>,
    representations: HashMap<NominalTypeId, NominalRepresentationAccess>,
}

impl SourceAccessTableBuilder {
    pub(crate) fn define_source(
        &mut self,
        source: SourceId,
        directly_visible: impl IntoIterator<Item = SourceId>,
    ) -> Result<(), FrontendBindingDefinitionError> {
        let mut visible = vec![source];
        visible.extend(directly_visible);
        match self.visible_sources.entry(source) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(visible);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(_) => Err(
                FrontendBindingDefinitionError::DuplicateSourceVisibility(source),
            ),
        }
    }

    pub(crate) fn define_source_module(
        &mut self,
        source: SourceId,
        module: ModuleId,
    ) -> Result<(), SourceOwnershipError> {
        match self.source_modules.entry(source) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(module);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                Err(SourceOwnershipError::ConflictingSource {
                    source,
                    existing: *entry.get(),
                    duplicate: module,
                })
            }
        }
    }

    pub(crate) fn define_site(
        &mut self,
        site: DeclarationSiteId,
        source: SourceId,
    ) -> Result<(), FrontendBindingDefinitionError> {
        match self.site_sources.entry(site) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(source);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                Err(FrontendBindingDefinitionError::DuplicateDeclarationSite {
                    site,
                    existing: *entry.get(),
                    duplicate: source,
                })
            }
        }
    }

    pub(crate) fn define_representation(
        &mut self,
        nominal: NominalTypeId,
        source: SourceId,
        contract_private: bool,
    ) -> Result<(), FrontendBindingDefinitionError> {
        let duplicate = NominalRepresentationAccess {
            source,
            contract_private,
        };
        match self.representations.entry(nominal) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(duplicate);
                Ok(())
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                let existing = *entry.get();
                Err(
                    FrontendBindingDefinitionError::DuplicateNominalRepresentation {
                        nominal,
                        existing_source: existing.source,
                        existing_contract_private: existing.contract_private,
                        duplicate_source: source,
                        duplicate_contract_private: contract_private,
                    },
                )
            }
        }
    }

    pub(crate) fn finish(self) -> SourceAccessTable {
        SourceAccessTable {
            visible_sources: self
                .visible_sources
                .into_iter()
                .map(|(source, mut visible)| {
                    visible.sort_unstable();
                    visible.dedup();
                    (source, visible.into_boxed_slice())
                })
                .collect(),
            ownership: SourceOwnershipTable {
                modules: self.source_modules,
            },
            site_sources: self.site_sources,
            representations: self.representations,
        }
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::ArenaBuilder;
    use nocter_source::{SourceMap, SourceName};

    use super::{SourceAccessTableBuilder, SourceOwnershipError};
    use crate::FrontendBindingDefinitionError;

    #[test]
    fn private_access_is_direct_and_directional() {
        let mut sources = SourceMap::new();
        let root = sources
            .add_bytes(SourceName::new("index.nct"), b"")
            .unwrap();
        let direct = sources
            .add_bytes(SourceName::new("direct.nct"), b"")
            .unwrap();
        let transitive = sources
            .add_bytes(SourceName::new("transitive.nct"), b"")
            .unwrap();
        let mut modules = ArenaBuilder::<nocter_model::ModuleId, ()>::new();
        let module = modules.insert(());
        let mut sites = ArenaBuilder::<nocter_model::DeclarationSiteId, ()>::new();
        let site = sites.insert(());
        let mut nominals = ArenaBuilder::<nocter_model::NominalTypeId, ()>::new();
        let nominal = nominals.insert(());
        let mut builder = SourceAccessTableBuilder::default();
        builder.define_source_module(root, module).unwrap();
        builder.define_source_module(direct, module).unwrap();
        builder.define_source_module(transitive, module).unwrap();
        assert!(matches!(
            builder.define_source_module(root, module),
            Err(SourceOwnershipError::ConflictingSource { source, .. }) if source == root
        ));
        builder.define_source(root, [direct]).unwrap();
        builder.define_source(direct, [transitive]).unwrap();
        builder.define_source(transitive, []).unwrap();
        builder.define_site(site, transitive).unwrap();
        builder
            .define_representation(nominal, transitive, true)
            .unwrap();
        assert_eq!(
            builder.define_source(root, []),
            Err(FrontendBindingDefinitionError::DuplicateSourceVisibility(
                root
            ))
        );
        assert!(matches!(
            builder.define_site(site, root),
            Err(FrontendBindingDefinitionError::DuplicateDeclarationSite {
                site: duplicate,
                existing,
                duplicate: duplicate_source,
            }) if duplicate == site && existing == transitive && duplicate_source == root
        ));
        assert!(matches!(
            builder.define_representation(nominal, root, false),
            Err(FrontendBindingDefinitionError::DuplicateNominalRepresentation {
                nominal: duplicate,
                existing_source,
                existing_contract_private: true,
                duplicate_source,
                duplicate_contract_private: false,
            }) if duplicate == nominal && existing_source == transitive && duplicate_source == root
        ));
        let access = builder.finish();

        assert!(!access.can_access_private(root, site).unwrap());
        assert_eq!(access.module_for_source(root).unwrap(), module);
        assert!(access.can_access_private(direct, site).unwrap());
        assert!(access.can_access_private(transitive, site).unwrap());
        assert!(!access.can_access_representation(root, nominal).unwrap());
        assert!(access.can_access_representation(direct, nominal).unwrap());
        assert!(access.representation_is_contract_private(nominal).unwrap());
    }
}
