use nocter_model::ExecutableItemId;

use crate::identity::MachineId;
use crate::linkage::MachineLinkagePlan;
use crate::{MachineDestructionId, MachineFunctionId, MachineLinkageId, MachineLinkageKey};

/// The structural one-to-one correspondence between linkage and machine-function identities.
///
/// Functions are emitted in the linkage table's dense order. This view derives the corresponding
/// function identity without retaining parallel maps that could disagree with that order.
#[derive(Clone, Copy)]
pub(crate) struct MachineFunctionDomain<'a> {
    linkage: &'a MachineLinkagePlan,
}

impl<'a> MachineFunctionDomain<'a> {
    pub(crate) const fn new(linkage: &'a MachineLinkagePlan) -> Self {
        Self { linkage }
    }

    pub(crate) fn for_linkage(self, linkage: MachineLinkageId) -> Option<MachineFunctionId> {
        self.linkage
            .get(linkage)
            .map(|_| MachineFunctionId::new(linkage.index()))
    }

    pub(crate) fn for_item(self, item: ExecutableItemId) -> Option<MachineFunctionId> {
        self.for_key(MachineLinkageKey::Item(item))
    }

    pub(crate) fn for_destruction(
        self,
        destruction: MachineDestructionId,
    ) -> Option<MachineFunctionId> {
        self.for_key(MachineLinkageKey::Destruction(destruction))
    }

    fn for_key(self, key: MachineLinkageKey) -> Option<MachineFunctionId> {
        self.linkage
            .id(key)
            .and_then(|linkage| self.for_linkage(linkage))
    }
}
