use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_mir::{MirBody, MirConstant, MirOperationKind, MirProgram, MirRoot};
use nocter_model::{ExecutableItemId, PackageTargetId, Symbol, TestId};

use crate::identity::{
    MachineDataId, MachineDestructionId, MachineId, MachineLinkageId, MachineTable,
};

/// Semantic owner of one emitted code symbol. Display spellings are not linkage identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MachineLinkageKey {
    Item(ExecutableItemId),
    ProcessRoot(PackageTargetId),
    TestRoot(TestId),
    Destruction(MachineDestructionId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineLinkageEntry {
    key: MachineLinkageKey,
}

impl MachineLinkageEntry {
    #[must_use]
    pub const fn key(self) -> MachineLinkageKey {
        self.key
    }
}

/// One compiler-owned test entry retained in declaration order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineTestLinkage {
    declaration: TestId,
    name: Symbol,
    test: MachineLinkageId,
    body: MachineLinkageId,
}

impl MachineTestLinkage {
    #[must_use]
    pub const fn declaration(self) -> TestId {
        self.declaration
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn test(self) -> MachineLinkageId {
        self.test
    }

    #[must_use]
    pub const fn body(self) -> MachineLinkageId {
        self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineRootLinkage {
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
pub struct MachineLinkageTable {
    entries: MachineTable<MachineLinkageId, MachineLinkageEntry>,
    ids: BTreeMap<MachineLinkageKey, MachineLinkageId>,
    root: MachineRootLinkage,
}

impl MachineLinkageTable {
    /// Closes every callable and compiler-owned root linkage identity.
    ///
    /// # Errors
    ///
    /// Rejects duplicate semantic keys or a root that refers to no executable item.
    pub fn build(program: &MirProgram) -> Result<Self, MachineLinkageError> {
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
        destructions: &crate::MachineDestructionTable,
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
    pub fn get(&self, id: MachineLinkageId) -> Option<MachineLinkageEntry> {
        self.entries.get(id).copied()
    }

    #[must_use]
    pub fn id(&self, key: MachineLinkageKey) -> Option<MachineLinkageId> {
        self.ids.get(&key).copied()
    }

    #[must_use]
    pub const fn root(&self) -> &MachineRootLinkage {
        &self.root
    }

    #[must_use]
    pub fn iter(
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
                        name: case.name(),
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

/// One canonical immutable byte string in the machine program.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineData {
    bytes: Box<[u8]>,
}

impl MachineData {
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Static data ordered by byte content rather than source discovery or first use.
#[derive(Debug)]
pub struct MachineDataTable {
    entries: MachineTable<MachineDataId, MachineData>,
    text_ids: BTreeMap<Box<str>, MachineDataId>,
}

impl MachineDataTable {
    #[must_use]
    pub fn build(program: &MirProgram) -> Self {
        let mut texts = BTreeSet::new();
        for (_, function) in program.functions().iter() {
            collect_text(function.body(), &mut texts);
        }
        match program.root() {
            MirRoot::Process(root) => collect_text(root.body(), &mut texts),
            MirRoot::Tests { cases, .. } => {
                for case in cases {
                    collect_text(case.body(), &mut texts);
                }
            }
        }

        let mut text_ids = BTreeMap::new();
        let entries = texts
            .into_iter()
            .enumerate()
            .map(|(index, text)| {
                let id = MachineDataId::new(index);
                let data = MachineData {
                    bytes: text.as_bytes().into(),
                };
                text_ids.insert(text, id);
                data
            })
            .collect::<Vec<_>>();
        Self {
            entries: MachineTable::from_values(entries),
            text_ids,
        }
    }

    #[must_use]
    pub fn get(&self, id: MachineDataId) -> Option<&MachineData> {
        self.entries.get(id)
    }

    #[must_use]
    pub fn text(&self, text: &str) -> Option<MachineDataId> {
        self.text_ids.get(text).copied()
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (MachineDataId, &MachineData)> {
        self.entries.iter()
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.len() == 0
    }
}

fn collect_text(body: &MirBody, texts: &mut BTreeSet<Box<str>>) {
    for (_, operation) in body.operations().iter() {
        if let MirOperationKind::Constant(MirConstant::Text(text)) = operation.kind() {
            texts.insert(text.clone());
        }
    }
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
