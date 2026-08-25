use std::collections::BTreeSet;

use nocter_model::{ConformanceId, ConstructionId, DropId, InstanceId};

/// Exact declaration containers admitted by declaration validation for editor-only analysis.
///
/// This value contains decisions; it does not evaluate declaration rules. The validation pass is
/// its only constructor, so recovery and checking cannot independently reinterpret a rejected
/// declaration graph.
#[derive(Debug)]
pub struct DeclarationAnalysisAdmission {
    constructions: BTreeSet<ConstructionId>,
    instances: BTreeSet<InstanceId>,
    conformances: BTreeSet<ConformanceId>,
    drops: BTreeSet<DropId>,
}

impl DeclarationAnalysisAdmission {
    pub(crate) fn new(
        constructions: BTreeSet<ConstructionId>,
        instances: BTreeSet<InstanceId>,
        conformances: BTreeSet<ConformanceId>,
        drops: BTreeSet<DropId>,
    ) -> Self {
        Self {
            constructions,
            instances,
            conformances,
            drops,
        }
    }

    #[must_use]
    pub fn admits_construction(&self, declaration: ConstructionId) -> bool {
        self.constructions.contains(&declaration)
    }

    #[must_use]
    pub fn admits_instance(&self, declaration: InstanceId) -> bool {
        self.instances.contains(&declaration)
    }

    #[must_use]
    pub fn admits_conformance(&self, declaration: ConformanceId) -> bool {
        self.conformances.contains(&declaration)
    }

    #[must_use]
    pub fn admits_drop(&self, declaration: DropId) -> bool {
        self.drops.contains(&declaration)
    }
}
