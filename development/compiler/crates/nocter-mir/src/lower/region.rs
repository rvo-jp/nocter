use nocter_model::{BodyNodeId, LocalBindingId, MirValueId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirPlaceRoot};

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
        let ty = self
            .builder
            .local_type(local)
            .ok_or(MirLoweringError::InvalidRegion(node))?;
        let value = self.append_value(ty, MirOperationKind::CreateRegion { parent })?;
        let destination = self.builder.add_place(MirPlaceRoot::Local(local), [], ty);
        self.append_effect(MirOperationKind::Initialize { destination, value })?;
        self.lower_node(body)?;
        Ok(None)
    }
}
