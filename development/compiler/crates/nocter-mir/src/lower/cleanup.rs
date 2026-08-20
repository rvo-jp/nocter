use nocter_checking::{
    CleanupCondition, CleanupPath, CleanupTarget, CleanupTiming, ConcreteDestructionKind,
    ConcreteDestructionPlan, PlaceRoot,
};
use nocter_model::{BodyNodeId, MirBlockId, MirPlaceId, TypeId};

use super::MirLoweringError;
use super::function::FunctionLowerer;
use crate::{
    MirBranchTarget, MirOperationKind, MirPlaceRoot, MirProjection, MirProjectionKind,
    MirSwitchCase, MirSwitchSubject, MirSwitchValue, MirTerminator,
};

impl FunctionLowerer<'_> {
    pub(super) fn lower_cleanup(
        &mut self,
        owner: BodyNodeId,
        timing: CleanupTiming,
    ) -> Result<(), MirLoweringError> {
        let lexical_regions = self.regions.clone();
        let result = self.lower_cleanup_actions(owner, timing);
        self.regions = lexical_regions;
        result
    }

    fn lower_cleanup_actions(
        &mut self,
        owner: BodyNodeId,
        timing: CleanupTiming,
    ) -> Result<(), MirLoweringError> {
        let actions = self
            .body
            .cleanups()
            .actions(owner, timing)
            .unwrap_or_default()
            .to_vec();
        for action in actions {
            if action.condition() == CleanupCondition::Always {
                self.lower_cleanup_target(owner, action.target())?;
                continue;
            }
            let flag = self.cleanup_flag(owner, action.target())?;
            let source = self
                .current
                .take()
                .ok_or(MirLoweringError::MissingCurrentBlock)?;
            let (initialized, _) = self.builder.create_block([]);
            let (join, _) = self.builder.create_block([]);
            self.builder.terminate(
                source,
                MirTerminator::BranchDropFlag {
                    flag,
                    initialized: MirBranchTarget::new(initialized, []),
                    uninitialized: MirBranchTarget::new(join, []),
                },
            )?;
            self.current = Some(initialized);
            self.lower_cleanup_target(owner, action.target())?;
            self.finish_cleanup_branch(join)?;
            self.current = Some(join);
        }
        Ok(())
    }

    fn lower_cleanup_target(
        &mut self,
        owner: BodyNodeId,
        target: &CleanupTarget,
    ) -> Result<(), MirLoweringError> {
        if let CleanupTarget::Region { binding, .. } = target {
            let region = self.ensure_local(*binding)?;
            self.append_effect(MirOperationKind::ReleaseRegion { region })?;
            if self.regions.pop() != Some(region) {
                return Err(MirLoweringError::InvalidRegion(owner));
            }
            return Ok(());
        }
        let place = match target {
            CleanupTarget::Path(path) => self.lower_cleanup_path(owner, path)?,
            CleanupTarget::Place { place, ty } => {
                let place = self.lower_place(*place)?;
                self.require_cleanup_place_type(owner, place, *ty)?;
                place
            }
            CleanupTarget::Value { node, ty } => {
                self.materialize_cleanup_value(owner, *node, *ty)?
            }
            CleanupTarget::EnumResidual { subject, ty, .. } => {
                self.materialize_cleanup_value(owner, *subject, *ty)?
            }
            CleanupTarget::Region { .. } => unreachable!("region cleanup returned above"),
        };
        let plan = self.item.body().cleanup_destruction(target).cloned();
        if let Some(plan) = plan {
            self.lower_destruction(owner, place, &plan)?;
        }
        self.mark_cleanup_complete(target)?;
        Ok(())
    }

    pub(super) fn lower_cleanup_path(
        &mut self,
        owner: BodyNodeId,
        path: &CleanupPath,
    ) -> Result<MirPlaceId, MirLoweringError> {
        if path.fields().len() != path.projection_types().len() {
            return Err(MirLoweringError::InvalidCleanup(owner));
        }
        let mut lowered = match path.root() {
            PlaceRoot::Parameter(parameter) => {
                let local = *self
                    .parameters
                    .get(&parameter)
                    .ok_or(MirLoweringError::InvalidCleanup(owner))?;
                let ty = self
                    .builder
                    .local_type(local)
                    .ok_or(MirLoweringError::InvalidCleanup(owner))?;
                super::place::LoweredPlacePath {
                    root: MirPlaceRoot::Local(local),
                    projections: Vec::new(),
                    ty,
                }
            }
            PlaceRoot::Local(local) => {
                let local = self.ensure_local(local)?;
                let ty = self
                    .builder
                    .local_type(local)
                    .ok_or(MirLoweringError::InvalidCleanup(owner))?;
                super::place::LoweredPlacePath {
                    root: MirPlaceRoot::Local(local),
                    projections: Vec::new(),
                    ty,
                }
            }
            PlaceRoot::Capture(capture) => self.lower_capture_path(capture)?,
        };
        for (field, source_ty) in path
            .fields()
            .iter()
            .copied()
            .zip(path.projection_types().iter().copied())
        {
            lowered.push(
                MirProjectionKind::Field(field),
                self.concrete_type(source_ty)?,
            );
        }
        let ty = self.concrete_type(path.ty())?;
        if lowered.ty != ty {
            return Err(MirLoweringError::InvalidCleanup(owner));
        }
        Ok(self
            .builder
            .add_place(lowered.root, lowered.projections, ty))
    }

    fn materialize_cleanup_value(
        &mut self,
        owner: BodyNodeId,
        node: BodyNodeId,
        source_ty: TypeId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let place = self.materialize_checked_value(node, source_ty)?;
        self.require_cleanup_place_type(owner, place, source_ty)?;
        Ok(place)
    }

    fn require_cleanup_place_type(
        &self,
        owner: BodyNodeId,
        place: MirPlaceId,
        source_ty: TypeId,
    ) -> Result<(), MirLoweringError> {
        let expected = self.concrete_type(source_ty)?;
        if self.builder.place(place).map(crate::MirPlace::ty) != Some(expected) {
            return Err(MirLoweringError::InvalidCleanup(owner));
        }
        Ok(())
    }

    pub(super) fn lower_destruction(
        &mut self,
        owner: BodyNodeId,
        place: MirPlaceId,
        plan: &ConcreteDestructionPlan,
    ) -> Result<(), MirLoweringError> {
        if self.builder.place(place).map(crate::MirPlace::ty) != Some(plan.ty()) {
            return Err(MirLoweringError::InvalidCleanup(owner));
        }
        match plan.kind() {
            ConcreteDestructionKind::Struct { drop, fields } => {
                if let Some(drop) = drop {
                    self.invoke_selected_drop(owner, place, drop)?;
                }
                for field in fields {
                    let child = self.project_cleanup_place(
                        owner,
                        place,
                        MirProjectionKind::Field(field.field()),
                        field.plan().ty(),
                    )?;
                    self.lower_destruction(owner, child, field.plan())?;
                }
            }
            ConcreteDestructionKind::Enum { drop, variants } => {
                if let Some(drop) = drop {
                    self.invoke_selected_drop(owner, place, drop)?;
                }
                self.lower_enum_destruction(owner, place, variants)?;
            }
            ConcreteDestructionKind::FixedArray { length, element } => {
                for index in (0..*length).rev() {
                    let child = self.project_cleanup_place(
                        owner,
                        place,
                        MirProjectionKind::FixedIndex(index),
                        element.ty(),
                    )?;
                    self.lower_destruction(owner, child, element)?;
                }
            }
            ConcreteDestructionKind::Optional(payload) => {
                self.lower_single_payload_destruction(
                    owner,
                    place,
                    MirSwitchValue::OptionalPresent,
                    MirProjectionKind::OptionalPayload,
                    payload,
                )?;
            }
            ConcreteDestructionKind::Fallible(payload) => {
                self.lower_single_payload_destruction(
                    owner,
                    place,
                    MirSwitchValue::FallibleSuccess,
                    MirProjectionKind::FallibleSuccess,
                    payload,
                )?;
            }
            ConcreteDestructionKind::Opaque {
                definition, plan, ..
            } => {
                let witness = self.project_cleanup_place(
                    owner,
                    place,
                    MirProjectionKind::OpaqueWitness(*definition),
                    plan.ty(),
                )?;
                self.lower_destruction(owner, witness, plan)?;
            }
            ConcreteDestructionKind::Closure(captures) => {
                for capture in captures {
                    let child = self.project_cleanup_place(
                        owner,
                        place,
                        MirProjectionKind::ClosureCapture(capture.capture()),
                        capture.plan().ty(),
                    )?;
                    self.lower_destruction(owner, child, capture.plan())?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn invoke_selected_drop(
        &mut self,
        owner: BodyNodeId,
        place: MirPlaceId,
        selection: &nocter_checking::DropSelection,
    ) -> Result<(), MirLoweringError> {
        let body = self
            .item
            .body()
            .drop_item(selection)
            .ok_or(MirLoweringError::InvalidCleanup(owner))?;
        self.append_effect(MirOperationKind::InvokeDrop {
            body,
            place,
            allocation: self.current_call_allocation(),
        })
    }

    fn project_cleanup_place(
        &mut self,
        owner: BodyNodeId,
        base: MirPlaceId,
        kind: MirProjectionKind,
        ty: TypeId,
    ) -> Result<MirPlaceId, MirLoweringError> {
        let base = self
            .builder
            .place(base)
            .cloned()
            .ok_or(MirLoweringError::InvalidCleanup(owner))?;
        let mut projections = base.projections().to_vec();
        projections.push(MirProjection::new(kind, ty));
        Ok(self.builder.add_place(base.root(), projections, ty))
    }

    fn lower_single_payload_destruction(
        &mut self,
        owner: BodyNodeId,
        place: MirPlaceId,
        value: MirSwitchValue,
        projection: MirProjectionKind,
        plan: &ConcreteDestructionPlan,
    ) -> Result<(), MirLoweringError> {
        let (payload, join) = self.begin_cleanup_switch(place, [value])?;
        self.current = Some(payload[0]);
        let child = self.project_cleanup_place(owner, place, projection, plan.ty())?;
        self.lower_destruction(owner, child, plan)?;
        self.finish_cleanup_branch(join)?;
        self.current = Some(join);
        Ok(())
    }

    fn lower_enum_destruction(
        &mut self,
        owner: BodyNodeId,
        place: MirPlaceId,
        variants: &[nocter_checking::ConcreteVariantDestruction],
    ) -> Result<(), MirLoweringError> {
        if variants.is_empty() {
            return Ok(());
        }
        let cases = variants
            .iter()
            .map(|variant| MirSwitchValue::Variant(variant.variant()))
            .collect::<Vec<_>>();
        let (blocks, join) = self.begin_cleanup_switch(place, cases)?;
        for (variant, block) in variants.iter().zip(blocks) {
            self.current = Some(block);
            for payload in variant.payload() {
                let child = self.project_cleanup_place(
                    owner,
                    place,
                    MirProjectionKind::VariantPayload {
                        variant: variant.variant(),
                        parameter: payload.parameter(),
                    },
                    payload.plan().ty(),
                )?;
                self.lower_destruction(owner, child, payload.plan())?;
            }
            self.finish_cleanup_branch(join)?;
        }
        self.current = Some(join);
        Ok(())
    }

    fn begin_cleanup_switch(
        &mut self,
        place: MirPlaceId,
        values: impl IntoIterator<Item = MirSwitchValue>,
    ) -> Result<(Vec<MirBlockId>, MirBlockId), MirLoweringError> {
        let source = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        let (join, _) = self.builder.create_block([]);
        let mut blocks = Vec::new();
        let mut mir_cases = Vec::new();
        for value in values {
            let (block, _) = self.builder.create_block([]);
            blocks.push(block);
            mir_cases.push(MirSwitchCase::new(value, MirBranchTarget::new(block, [])));
        }
        self.builder.terminate(
            source,
            MirTerminator::Switch {
                subject: MirSwitchSubject::Place(place),
                cases: mir_cases.into_boxed_slice(),
                fallback: MirBranchTarget::new(join, []),
            },
        )?;
        Ok((blocks, join))
    }

    fn finish_cleanup_branch(&mut self, join: MirBlockId) -> Result<(), MirLoweringError> {
        let block = self
            .current
            .take()
            .ok_or(MirLoweringError::MissingCurrentBlock)?;
        self.builder
            .terminate(block, MirTerminator::Goto(MirBranchTarget::new(join, [])))?;
        Ok(())
    }
}
