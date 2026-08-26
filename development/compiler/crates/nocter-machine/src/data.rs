use std::collections::{BTreeMap, BTreeSet};

use nocter_mir::{MirBody, MirConstant, MirOperationKind, MirProgram, MirRoot};

use crate::MachineDataId;
use crate::identity::{MachineId, MachineTable};

/// One canonical immutable byte string in the final machine program.
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

/// Final static data ordered by byte content rather than source discovery or first use.
#[derive(Debug)]
pub struct MachineDataTable {
    entries: MachineTable<MachineDataId, MachineData>,
}

impl MachineDataTable {
    #[must_use]
    pub fn get(&self, id: MachineDataId) -> Option<&MachineData> {
        self.entries.get(id)
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

/// Construction-only lookup from MIR text constants to final data identities.
///
/// Lowering consumes this plan and transfers only the dense byte table into [`MachineDataTable`].
#[derive(Debug)]
pub(crate) struct MachineDataPlan {
    entries: MachineTable<MachineDataId, MachineData>,
    text_ids: BTreeMap<Box<str>, MachineDataId>,
}

impl MachineDataPlan {
    pub(crate) fn build(program: &MirProgram) -> Self {
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

    pub(crate) fn text(&self, text: &str) -> Option<MachineDataId> {
        self.text_ids.get(text).copied()
    }

    pub(crate) fn finish(self) -> MachineDataTable {
        MachineDataTable {
            entries: self.entries,
        }
    }
}

fn collect_text(body: &MirBody, texts: &mut BTreeSet<Box<str>>) {
    for (_, operation) in body.operations().iter() {
        if let MirOperationKind::Constant(MirConstant::Text(text)) = operation.kind() {
            texts.insert(text.clone());
        }
    }
}
