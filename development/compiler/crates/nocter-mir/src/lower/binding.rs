use nocter_checking::CheckedBindingPattern;
use nocter_model::{BodyNodeId, MirPlaceId, MirValueId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{MirOperationKind, MirPlaceRoot, MirProjectionKind, MirReadMode};

impl FunctionLowerer<'_> {
    pub(super) fn lower_binding(
        &mut self,
        owner: BodyNodeId,
        pattern: &CheckedBindingPattern,
        initializer: BodyNodeId,
    ) -> Result<(), MirLoweringError> {
        if let CheckedBindingPattern::Local { binding, ty } = pattern {
            let value = self.require_value(initializer)?;
            return self.initialize_binding(*binding, *ty, value);
        }

        let value = self.require_value(initializer)?;
        let place = self.materialize_value_storage(initializer, value)?;
        self.lower_binding_pattern(owner, pattern, place)?;
        self.lower_cleanup(owner, nocter_checking::CleanupTiming::DuringBinding)
    }

    fn lower_binding_pattern(
        &mut self,
        owner: BodyNodeId,
        pattern: &CheckedBindingPattern,
        place: MirPlaceId,
    ) -> Result<(), MirLoweringError> {
        match pattern {
            CheckedBindingPattern::Local { binding, ty } => {
                let source_ty = *ty;
                let ty = self.concrete_type(source_ty)?;
                let value = self.append_value(
                    ty,
                    MirOperationKind::Read {
                        place,
                        mode: MirReadMode::Move,
                    },
                )?;
                self.initialize_binding(*binding, source_ty, value)
            }
            CheckedBindingPattern::Discard { .. } => Ok(()),
            CheckedBindingPattern::Tuple { elements, .. } => {
                for (index, element) in elements.iter().enumerate() {
                    let child = self.project_cleanup_place(
                        owner,
                        place,
                        MirProjectionKind::TupleElement(index),
                        self.concrete_type(element.ty())?,
                    )?;
                    self.lower_binding_pattern(owner, element, child)?;
                }
                Ok(())
            }
        }
    }

    fn initialize_binding(
        &mut self,
        binding: nocter_model::LocalBindingId,
        source_ty: nocter_model::TypeId,
        value: MirValueId,
    ) -> Result<(), MirLoweringError> {
        let local = self.ensure_local(binding)?;
        let ty = self.concrete_type(source_ty)?;
        let place = self.builder.add_place(MirPlaceRoot::Local(local), [], ty);
        self.append_effect(MirOperationKind::Initialize {
            destination: place,
            value,
        })?;
        self.mark_binding_initialized(binding)
    }
}
