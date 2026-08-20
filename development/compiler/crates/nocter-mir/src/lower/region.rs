use nocter_model::{BodyNodeId, LocalBindingId, MirValueId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::MirOperationKind;

impl FunctionLowerer<'_> {
    pub(super) fn lower_region(
        &mut self,
        node: BodyNodeId,
        binding: LocalBindingId,
        allocator: BodyNodeId,
        body: BodyNodeId,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let parent = self.lower_place_carrier(allocator)?;
        let local = self.ensure_local(binding)?;
        self.builder
            .local_type(local)
            .ok_or(MirLoweringError::InvalidRegion(node))?;
        self.append_effect(MirOperationKind::CreateRegion {
            parent,
            region: local,
        })?;
        self.regions.push(local);
        let lowered = self.lower_node(body);
        let active = self.regions.pop();
        if active != Some(local) {
            return Err(MirLoweringError::InvalidRegion(node));
        }
        lowered?;
        Ok(None)
    }
}
