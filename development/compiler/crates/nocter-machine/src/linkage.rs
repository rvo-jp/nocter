use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_mir::{MirProgram, MirRoot};
use nocter_model::{ExecutableItemId, PackageTargetId, TestId};

use crate::identity::{MachineDestructionId, MachineId, MachineLinkageId, MachineTable};

/// Semantic owner of one emitted code symbol. Display spellings are not linkage identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MachineLinkageKey {
    Item(ExecutableItemId),
    ProcessRoot(PackageTargetId),
    TestRoot(TestId),
    Destruction(MachineDestructionId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MachineLinkageEntry {
    key: MachineLinkageKey,
}

impl MachineLinkageEntry {
    #[must_use]
    pub(crate) const fn key(self) -> MachineLinkageKey {
        self.key
    }
}

/// One compiler-owned test entry retained in declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MachineTestLinkage {
    declaration: TestId,
    name: Box<str>,
    test: MachineLinkageId,
    body: MachineLinkageId,
}

impl MachineTestLinkage {
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn declaration(&self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub(crate) const fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub(crate) const fn test(&self) -> MachineLinkageId {
        self.test
    }

    #[must_use]
    pub(crate) const fn body(&self) -> MachineLinkageId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MachineRootLinkage {
    Process {
        target: PackageTargetId,
        process: MachineLinkageId,
        entry: MachineLinkageId,
    },
    Tests {
        target: PackageTargetId,
        cases: Box<[MachineTestLinkage]>,
    },
}

/// Deterministic code linkage projected only from dense semantic owners.
#[derive(Debug)]
pub(crate) struct MachineLinkagePlan {
    entries: MachineTable<MachineLinkageId, MachineLinkageEntry>,
    ids: BTreeMap<MachineLinkageKey, MachineLinkageId>,
    root: MachineRootLinkage,
}

impl MachineLinkagePlan {
    /// Closes every callable and compiler-owned root linkage identity.
    ///
    /// # Errors
    ///
    /// Rejects duplicate semantic keys or a root that refers to no executable item.
    pub(crate) fn build(program: &MirProgram) -> Result<Self, MachineLinkageError> {
        let mut keys = BTreeSet::new();
        for (item, _) in program.functions().iter() {
            if !keys.insert(MachineLinkageKey::Item(item)) {
                return Err(MachineLinkageError::DuplicateKey(MachineLinkageKey::Item(
                    item,
                )));
            }
        }
        match program.root() {
            MirRoot::Process(root) => {
                keys.insert(MachineLinkageKey::ProcessRoot(root.target()));
            }
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    if !keys.insert(MachineLinkageKey::TestRoot(case.declaration())) {
                        return Err(MachineLinkageError::DuplicateKey(
                            MachineLinkageKey::TestRoot(case.declaration()),
                        ));
                    }
                }
            }
        }

        let mut ids = BTreeMap::new();
        let entries = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| {
                let id = MachineLinkageId::new(index);
                ids.insert(key, id);
                MachineLinkageEntry { key }
            })
            .collect::<Vec<_>>();
        let root = close_root(program.root(), &ids)?;
        Ok(Self {
            entries: MachineTable::from_values(entries),
            ids,
            root,
        })
    }

    pub(crate) fn with_destructions(
        mut self,
        destructions: &crate::destruction_table::MachineDestructionPlanTable,
    ) -> Result<Self, MachineLinkageError> {
        let mut entries = self.entries.values().to_vec();
        for (destruction, _) in destructions.iter() {
            let key = MachineLinkageKey::Destruction(destruction);
            let id = MachineLinkageId::new(entries.len());
            if self.ids.insert(key, id).is_some() {
                return Err(MachineLinkageError::DuplicateKey(key));
            }
            entries.push(MachineLinkageEntry { key });
        }
        self.entries = MachineTable::from_values(entries);
        Ok(self)
    }

    #[must_use]
    pub(crate) fn get(&self, id: MachineLinkageId) -> Option<MachineLinkageEntry> {
        self.entries.get(id).copied()
    }

    #[must_use]
    pub(crate) fn id(&self, key: MachineLinkageKey) -> Option<MachineLinkageId> {
        self.ids.get(&key).copied()
    }

    #[must_use]
    pub(crate) const fn root(&self) -> &MachineRootLinkage {
        &self.root
    }

    #[must_use]
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = (MachineLinkageId, MachineLinkageEntry)> + '_ {
        self.entries.iter().map(|(id, entry)| (id, *entry))
    }
}

fn close_root(
    root: &MirRoot,
    ids: &BTreeMap<MachineLinkageKey, MachineLinkageId>,
) -> Result<MachineRootLinkage, MachineLinkageError> {
    match root {
        MirRoot::Process(root) => Ok(MachineRootLinkage::Process {
            target: root.target(),
            process: require_id(ids, MachineLinkageKey::ProcessRoot(root.target()))?,
            entry: require_id(ids, MachineLinkageKey::Item(root.entry()))?,
        }),
        MirRoot::Tests { target, cases } => {
            let cases = cases
                .iter()
                .map(|case| {
                    Ok(MachineTestLinkage {
                        declaration: case.declaration(),
                        name: case.name().into(),
                        test: require_id(ids, MachineLinkageKey::TestRoot(case.declaration()))?,
                        body: require_id(ids, MachineLinkageKey::Item(case.item()))?,
                    })
                })
                .collect::<Result<Vec<_>, MachineLinkageError>>()?
                .into_boxed_slice();
            Ok(MachineRootLinkage::Tests {
                target: *target,
                cases,
            })
        }
    }
}

fn require_id(
    ids: &BTreeMap<MachineLinkageKey, MachineLinkageId>,
    key: MachineLinkageKey,
) -> Result<MachineLinkageId, MachineLinkageError> {
    ids.get(&key)
        .copied()
        .ok_or(MachineLinkageError::MissingKey(key))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MachineLinkageError {
    DuplicateKey(MachineLinkageKey),
    MissingKey(MachineLinkageKey),
}

impl fmt::Display for MachineLinkageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "machine linkage construction failed: {self:?}")
    }
}

impl std::error::Error for MachineLinkageError {}
