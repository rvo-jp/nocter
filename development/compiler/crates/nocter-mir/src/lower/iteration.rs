use nocter_checking::{CheckedIteratorAcquisition, IterationAcquisition};
use nocter_model::{BodyNodeId, LocalBindingId, MirLocalId, MirValueId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirPlaceRoot, MirProjection, MirProjectionKind, MirReadMode};

impl FunctionLowerer<'_> {
    pub(super) fn bind_optional_iteration_item(
        &mut self,
        optional: MirLocalId,
        item: TypeId,
        binding: LocalBindingId,
    ) -> Result<(), MirLoweringError> {
        let item_place = self.builder.add_place(
            MirPlaceRoot::Local(optional),
            [MirProjection::new(MirProjectionKind::OptionalPayload, item)],
            item,
        );
        let value = self.append_value(
            item,
            MirOperationKind::Read {
                place: item_place,
                mode: MirReadMode::Move,
            },
        )?;
        let binding_local = self.ensure_local(binding)?;
        let destination = self
            .builder
            .add_place(MirPlaceRoot::Local(binding_local), [], item);
        self.append_effect(MirOperationKind::Initialize { destination, value })?;
        self.mark_binding_initialized(binding)
    }

    pub(super) fn lower_iterator_acquisition(
        &mut self,
        node: BodyNodeId,
        ty: TypeId,
        acquisition: &CheckedIteratorAcquisition,
    ) -> Result<MirValueId, MirLoweringError> {
        match acquisition.acquisition() {
            IterationAcquisition::Direct => self.lower_receiver(node, acquisition.source(), ty),
            IterationAcquisition::Expansion(selection) => {
                let step = self.invocation_step(node, selection)?;
                let signature = self.step_signature(&step)?;
                let [input] = signature.parameters() else {
                    return Err(MirLoweringError::InvalidDispatch(node));
                };
                if signature.result() != ty {
                    return Err(MirLoweringError::InvalidDispatch(node));
                }
                let source = self.lower_receiver(node, acquisition.source(), *input)?;
                self.emit_dispatch_step(node, ty, &step, [source])
            }
        }
    }
}
