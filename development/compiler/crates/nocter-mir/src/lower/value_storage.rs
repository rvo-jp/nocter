use nocter_model::{BodyNodeId, MirPlaceId, MirValueId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirLocalKind, MirOperationKind, MirPlaceRoot};

impl FunctionLowerer<'_> {
    /// Returns the sole addressable storage slot for one checked value and initializes it once.
    ///
    /// Borrow preparation, outcome inspection, pattern projection, and cleanup must share this
    /// path. Creating independent slots would duplicate ownership even if their SSA input happened
    /// to carry the same identity.
    pub(super) fn materialize_value_storage(
        &mut self,
        node: BodyNodeId,
        value: MirValueId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let ty = self
            .builder
            .value_type(value)
            .ok_or(MirLoweringError::UnknownValue(value))?;
        let place = self.reserve_value_storage(node, ty)?;
        if self.initialized_value_storage.insert(node) {
            self.append_effect(MirOperationKind::Initialize {
                destination: place,
                value,
            })?;
            self.mark_value_storage_initialized(node)?;
        }
        Ok(place)
    }

    pub(super) fn materialize_checked_value(
        &mut self,
        node: BodyNodeId,
        source_ty: TypeId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let value = self
            .values
            .get(&node)
            .copied()
            .ok_or(MirLoweringError::InvalidCleanup(node))?;
        let expected = self.concrete_type(source_ty)?;
        if self.builder.value_type(value) != Some(expected) {
            return Err(MirLoweringError::InvalidCleanup(node));
        }
        self.materialize_value_storage(node, value)
    }

    pub(super) fn reserve_value_storage(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        if let Some(place) = self.value_storage.get(&node).copied() {
            if self.builder.place(place).map(crate::MirPlace::ty) != Some(ty) {
                return Err(MirLoweringError::InvalidCleanup(node));
            }
            return Ok(place);
        }
        let local = self.builder.add_local(ty, MirLocalKind::Temporary, true);
        let place = self.builder.add_place(MirPlaceRoot::Local(local), [], ty);
        self.value_storage.insert(node, place);
        Ok(place)
    }
}
