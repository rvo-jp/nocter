use nocter_checking::{
    CheckedPattern, CheckedPatternArm, CheckedPatternFallback, CheckedPatternSubject,
    PatternBindingMode, PatternRemainder, PatternSubjectPreparation,
};
use nocter_model::{
    BodyNodeId, BuiltinType, MirPlaceId, MirValueId, NominalTypeId, TypeId, TypeKind,
};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirBranchTarget, MirOperationKind, MirPlaceRoot, MirProjection, MirProjectionKind, MirReadMode,
    MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_pattern(
        &mut self,
        node: BodyNodeId,
        subject: CheckedPatternSubject,
        arms: &[CheckedPatternArm],
        fallback: Option<CheckedPatternFallback>,
        unmatched: bool,
    ) -> Result<Option<MirValueId>, MirLoweringError> {
        let subject_place = self.lower_pattern_subject(node, subject)?;
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let arm_blocks = arms
            .iter()
            .map(|_| self.builder.create_block([]).0)
            .collect::<Vec<_>>();
        let (fallback_block, _) = self.builder.create_block([]);
        let cases = arms
            .iter()
            .zip(arm_blocks.iter().copied())
            .map(|(arm, block)| {
                MirSwitchCase::new(
                    MirSwitchValue::Variant(arm.pattern().variant()),
                    MirBranchTarget::new(block, []),
                )
            })
            .collect::<Vec<_>>();
        self.builder.terminate(
            source,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(subject_place),
                cases: cases.into_boxed_slice(),
                fallback: MirBranchTarget::new(fallback_block, []),
            },
        )?;

        let mut exits =
            Vec::with_capacity(arms.len() + usize::from(fallback.is_some() || unmatched));
        for (arm, block) in arms.iter().zip(arm_blocks) {
            self.current = Some(block);
            self.prepare_pattern_arm(node, subject, subject_place, arm.pattern())?;
            let value = self.lower_node(arm.body())?;
            exits.push(self.current.map(|block| (block, value)));
        }

        if let Some(fallback) = fallback.filter(|fallback| fallback.reachable()) {
            self.current = Some(fallback_block);
            self.select_pattern_fallback(subject)?;
            let value = self.lower_node(fallback.body())?;
            exits.push(self.current.map(|block| (block, value)));
        } else if unmatched {
            self.current = Some(fallback_block);
            self.select_pattern_fallback(subject)?;
            exits.push(Some((fallback_block, None)));
        } else {
            self.builder
                .terminate(fallback_block, MirTerminator::Unreachable)?;
        }

        let source_ty = self
            .body
            .nodes()
            .get(node)
            .map(nocter_checking::CheckedNode::ty)
            .ok_or(MirLoweringError::UnknownNode(node))?;
        let ty = self.concrete_type(source_ty)?;
        let carries_value = !matches!(
            self.executable.types().get(ty),
            Some(TypeKind::Builtin(BuiltinType::Void | BuiltinType::Never))
        );
        self.join_branches(ty, carries_value, exits)
    }

    fn lower_pattern_subject(
        &mut self,
        node: BodyNodeId,
        subject: CheckedPatternSubject,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let place_carrier = matches!(
            self.body
                .nodes()
                .get(subject.value())
                .map(nocter_checking::CheckedNode::operation),
            Some(nocter_checking::CheckedOperation::Place(_))
        );
        let place = match subject.preparation() {
            PatternSubjectPreparation::RetainedPlace => self.lower_place_node(subject.value())?,
            PatternSubjectPreparation::OwnedTemporary
            | PatternSubjectPreparation::ConsumedPlace => {
                let value = self.require_value(subject.value())?;
                self.materialize_value_storage(subject.value(), value)?
            }
            PatternSubjectPreparation::Borrowed(capability) => {
                let value = self.lower_place_carrier(subject.value())?;
                let borrow = self
                    .builder
                    .value_type(value)
                    .ok_or(MirLoweringError::InvalidPattern(node))?;
                let Some(TypeKind::Borrow {
                    capability: actual,
                    referent,
                }) = self.executable.types().get(borrow)
                else {
                    return Err(MirLoweringError::InvalidPattern(node));
                };
                if *actual != capability {
                    return Err(MirLoweringError::InvalidPattern(node));
                }
                self.builder.add_place(
                    MirPlaceRoot::Dereference { value, capability },
                    [],
                    *referent,
                )
            }
        };
        if place_carrier {
            self.lower_cleanup(
                subject.value(),
                nocter_checking::CleanupTiming::AtControlHeaderEnd,
            )?;
        }
        self.require_pattern_nominal(node, place, subject.nominal())?;
        Ok(place)
    }

    fn require_pattern_nominal(
        &self,
        node: BodyNodeId,
        place: MirPlaceId,
        expected: NominalTypeId,
    ) -> Result<(), MirLoweringError> {
        let ty = self
            .builder
            .place(place)
            .map(crate::MirPlace::ty)
            .ok_or(MirLoweringError::InvalidPattern(node))?;
        if !matches!(
            self.executable.types().get(ty),
            Some(TypeKind::Nominal { definition, .. }) if *definition == expected
        ) {
            return Err(MirLoweringError::InvalidPattern(node));
        }
        Ok(())
    }

    fn prepare_pattern_arm(
        &mut self,
        node: BodyNodeId,
        subject: CheckedPatternSubject,
        subject_place: MirPlaceId,
        pattern: &CheckedPattern,
    ) -> Result<(), MirLoweringError> {
        if matches!(
            subject.preparation(),
            PatternSubjectPreparation::OwnedTemporary | PatternSubjectPreparation::ConsumedPlace
        ) {
            self.select_pattern_remainder(
                subject.value(),
                Some(pattern.variant()),
                pattern.remainder(),
            )?;
        } else if pattern.remainder() != &PatternRemainder::NoCleanup {
            return Err(MirLoweringError::InvalidPattern(node));
        }
        if let Some(drop) = pattern.before_transfer_drop() {
            self.invoke_selected_drop(node, subject_place, drop)?;
        }
        for slot in pattern.slots() {
            let (Some(binding), Some(mode)) = (slot.binding(), slot.binding_mode()) else {
                if slot.binding().is_some() || slot.binding_mode().is_some() {
                    return Err(MirLoweringError::InvalidPattern(node));
                }
                continue;
            };
            let local = self.ensure_local(binding)?;
            let binding_ty = self
                .builder
                .local_type(local)
                .ok_or(MirLoweringError::InvalidPattern(node))?;
            let payload_ty = match mode {
                PatternBindingMode::Copy | PatternBindingMode::Move => binding_ty,
                PatternBindingMode::Borrow(capability) => {
                    match self.executable.types().get(binding_ty) {
                        Some(TypeKind::Borrow {
                            capability: actual,
                            referent,
                        }) if *actual == capability => *referent,
                        _ => return Err(MirLoweringError::InvalidPattern(node)),
                    }
                }
            };
            let payload = self.project_pattern_payload(
                node,
                subject_place,
                pattern.variant(),
                slot.parameter(),
                payload_ty,
            )?;
            let value = match mode {
                PatternBindingMode::Copy => self.append_value(
                    binding_ty,
                    MirOperationKind::Read {
                        place: payload,
                        mode: MirReadMode::Copy,
                    },
                )?,
                PatternBindingMode::Move => self.append_value(
                    binding_ty,
                    MirOperationKind::Read {
                        place: payload,
                        mode: MirReadMode::Move,
                    },
                )?,
                PatternBindingMode::Borrow(capability) => self.append_value(
                    binding_ty,
                    MirOperationKind::Borrow {
                        place: payload,
                        capability,
                    },
                )?,
            };
            let destination = self
                .builder
                .add_place(MirPlaceRoot::Local(local), [], binding_ty);
            self.append_effect(MirOperationKind::Initialize { destination, value })?;
            self.mark_binding_initialized(binding)?;
        }
        Ok(())
    }

    fn select_pattern_fallback(
        &mut self,
        subject: CheckedPatternSubject,
    ) -> Result<(), MirLoweringError> {
        if matches!(
            subject.preparation(),
            PatternSubjectPreparation::OwnedTemporary | PatternSubjectPreparation::ConsumedPlace
        ) {
            self.select_pattern_remainder(subject.value(), None, &PatternRemainder::Complete)?;
        }
        Ok(())
    }

    fn project_pattern_payload(
        &mut self,
        node: BodyNodeId,
        base: MirPlaceId,
        variant: nocter_model::VariantId,
        parameter: nocter_model::ParameterId,
        ty: TypeId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let base = self
            .builder
            .place(base)
            .cloned()
            .ok_or(MirLoweringError::InvalidPattern(node))?;
        let mut projections = base.projections().to_vec();
        projections.push(MirProjection::new(
            MirProjectionKind::VariantPayload { variant, parameter },
            ty,
        ));
        Ok(self.builder.add_place(base.root(), projections, ty))
    }
}
