use nocter_checking::{CheckedIteratorAcquisition, IterationAcquisition};
use nocter_model::{BodyNodeId, MirValueId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;

impl FunctionLowerer<'_> {
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
