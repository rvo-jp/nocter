use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{BodyNodeId, BodyScopeId, PlaceId, TypeId, TypeKind};

use super::super::error::BodyCheckInternalError;
use crate::checked::CleanupEffect;
use crate::copyability::{CopyProofs, Copyability};
use crate::ownership::{
    InitializationState, MovePath, MoveProjection, OwnershipState, owned_body_roots,
};
use crate::type_relations::TypeSubstitution;
use crate::{
    BodySource, CheckedBody, CheckedPattern, CleanupAction, CleanupCondition, CleanupPath,
    CleanupProjection, CleanupTarget, ClosureTable, DropTable, LocalBindingKind, PatternRemainder,
    PlaceRoot,
};

use super::destruction_effect::DestructionEffectResolver;

pub(super) struct CleanupPlanner<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut nocter_model::TypeTransaction,
    copyabilities: &'program mut crate::copyability::CopyabilityTransaction,
    drops: &'program DropTable,
    closures: &'program ClosureTable,
    opaque_witnesses: &'program crate::OpaqueWitnessTable,
    body: &'program CheckedBody,
    source: BodySource<'program>,
    copy_proofs: &'program CopyProofs,
}

#[derive(Clone, Copy)]
pub(super) struct CleanupContext<'program> {
    graph: &'program DeclarationGraph,
    drops: &'program DropTable,
    closures: &'program ClosureTable,
    opaque_witnesses: &'program crate::OpaqueWitnessTable,
    body: &'program CheckedBody,
    source: BodySource<'program>,
    copy_proofs: &'program CopyProofs,
}

impl<'program> CleanupContext<'program> {
    pub(super) const fn new(
        graph: &'program DeclarationGraph,
        drops: &'program DropTable,
        closures: &'program ClosureTable,
        opaque_witnesses: &'program crate::OpaqueWitnessTable,
        body: &'program CheckedBody,
        source: BodySource<'program>,
        copy_proofs: &'program CopyProofs,
    ) -> Self {
        Self {
            graph,
            drops,
            closures,
            opaque_witnesses,
            body,
            source,
            copy_proofs,
        }
    }
}

impl<'program> CleanupPlanner<'program> {
    pub(super) fn new(
        types: &'program mut nocter_model::TypeTransaction,
        copyabilities: &'program mut crate::copyability::CopyabilityTransaction,
        context: CleanupContext<'program>,
    ) -> Self {
        Self {
            graph: context.graph,
            types,
            copyabilities,
            drops: context.drops,
            closures: context.closures,
            opaque_witnesses: context.opaque_witnesses,
            body: context.body,
            source: context.source,
            copy_proofs: context.copy_proofs,
        }
    }

    pub(super) fn scope_actions(
        &mut self,
        scope: BodyScopeId,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut locals = self
            .body
            .locals()
            .iter()
            .filter(|(_, local)| {
                local.declaration().scope() == scope
                    && local.declaration().kind() != LocalBindingKind::Region
            })
            .map(|(local, checked)| (local, checked.ty()))
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        while let Some((local, ty)) = locals.pop() {
            let root = PlaceRoot::Local(local);
            if !state.contains_root(root) {
                continue;
            }
            self.plan_path(&MovePath::root(root), ty, state, &mut actions)?;
            state.forget_root(root);
        }
        Ok(actions)
    }

    pub(super) fn parameter_actions(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut roots = owned_body_roots(self.graph, self.source).ok_or(
            BodyCheckInternalError::BodyIdentityMismatch(self.source.body()),
        )?;
        let mut actions = Vec::new();
        while let Some(root) = roots.pop() {
            let ty = self.root_type(root)?;
            self.plan_path(&MovePath::root(root), ty, state, &mut actions)?;
            state.forget_root(root);
        }
        Ok(actions)
    }

    pub(super) fn closure_capture_actions(
        &mut self,
        closure: &crate::ClosureDefinition,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut actions = Vec::new();
        for capture in closure.environment().iter().rev().copied() {
            let binding = capture.binding();
            let checked = self
                .body
                .captures()
                .get(binding)
                .ok_or(BodyCheckInternalError::CleanupPlanning)?;
            if checked.declaration().mode() != crate::CaptureMode::Move {
                continue;
            }
            let root = PlaceRoot::Capture(binding);
            self.plan_path(&MovePath::root(root), capture.ty(), state, &mut actions)?;
            state.forget_root(root);
        }
        Ok(actions)
    }

    pub(super) fn value_action(
        &mut self,
        node: nocter_model::BodyNodeId,
        ty: TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        if !self.needs_cleanup(ty)? {
            return Ok(None);
        }
        let effect = self.destruction_effect(ty)?;
        Ok(Some(CleanupAction::new(
            CleanupTarget::Value { node, ty },
            CleanupCondition::Always,
            effect,
        )))
    }

    pub(super) fn enum_residual_action(
        &mut self,
        subject: BodyNodeId,
        ty: TypeId,
        pattern: &CheckedPattern,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        match pattern.remainder() {
            PatternRemainder::NoCleanup => Ok(None),
            PatternRemainder::Complete => self.value_action(subject, ty),
            PatternRemainder::Residual(payload) => {
                let effect = self.enum_residual_effect(ty, pattern.variant(), payload)?;
                Ok(Some(CleanupAction::new(
                    CleanupTarget::EnumResidual {
                        subject,
                        variant: pattern.variant(),
                        payload: payload.clone(),
                        ty,
                    },
                    CleanupCondition::Always,
                    effect,
                )))
            }
        }
    }

    pub(super) fn explicit_path_action(
        &mut self,
        path: &MovePath,
        ty: TypeId,
    ) -> Result<CleanupAction, BodyCheckInternalError> {
        if !self.needs_cleanup(ty)? {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        Ok(CleanupAction::new(
            CleanupTarget::Path(self.checked_cleanup_path(path, ty)?),
            CleanupCondition::Always,
            self.destruction_effect(ty)?,
        ))
    }

    pub(super) fn binding_discard_action(
        &mut self,
        path: &MovePath,
        ty: TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        if !self.needs_cleanup(ty)? {
            return Ok(None);
        }
        Ok(Some(CleanupAction::new(
            CleanupTarget::Path(self.checked_cleanup_path(path, ty)?),
            CleanupCondition::Always,
            self.destruction_effect(ty)?,
        )))
    }

    pub(super) fn replacement_path_actions(
        &mut self,
        path: &MovePath,
        ty: TypeId,
        state: &OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut actions = Vec::new();
        self.plan_path(path, ty, state, &mut actions)?;
        Ok(actions)
    }

    pub(super) fn replacement_place_action(
        &mut self,
        place: PlaceId,
        ty: TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        if !self.needs_cleanup(ty)? {
            return Ok(None);
        }
        let effect = self.destruction_effect(ty)?;
        Ok(Some(CleanupAction::new(
            CleanupTarget::Place { place, ty },
            CleanupCondition::Always,
            effect,
        )))
    }

    fn plan_path(
        &mut self,
        path: &MovePath,
        ty: TypeId,
        state: &OwnershipState,
        actions: &mut Vec<CleanupAction>,
    ) -> Result<(), BodyCheckInternalError> {
        if state.has_descendant(path) {
            let children = self.aggregate_children(ty)?;
            for (projection, child_ty) in children.into_iter().rev() {
                let child = match projection {
                    MoveProjection::Field(field) => path.field(field),
                    MoveProjection::TupleElement(index) => path.tuple_element(index),
                };
                self.plan_path(&child, child_ty, state, actions)?;
            }
            return Ok(());
        }
        if !self.needs_cleanup(ty)? {
            return Ok(());
        }
        let condition = match state
            .initialization(path)
            .map_err(|_| BodyCheckInternalError::OwnershipState)?
        {
            InitializationState::Initialized => CleanupCondition::Always,
            InitializationState::MaybeInitialized => CleanupCondition::IfInitialized,
            InitializationState::Uninitialized => return Ok(()),
        };
        let target = CleanupTarget::Path(self.checked_cleanup_path(path, ty)?);
        let effect = self.destruction_effect(ty)?;
        actions.push(CleanupAction::new(target, condition, effect));
        Ok(())
    }

    fn checked_cleanup_path(
        &mut self,
        path: &MovePath,
        expected: TypeId,
    ) -> Result<CleanupPath, BodyCheckInternalError> {
        let root_ty = self.root_type(path.root_identity())?;
        let mut current = root_ty;
        let mut projections = Vec::with_capacity(path.projections().len());
        for projection in path.projections() {
            let next = self
                .aggregate_children(current)?
                .into_iter()
                .find_map(|(candidate, ty)| (candidate == *projection).then_some(ty))
                .ok_or(BodyCheckInternalError::CleanupPlanning)?;
            projections.push(match projection {
                MoveProjection::Field(field) => CleanupProjection::named_field(*field, next),
                MoveProjection::TupleElement(index) => {
                    CleanupProjection::tuple_element(*index, next)
                }
            });
            current = next;
        }
        if current != expected {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        Ok(CleanupPath::new(path.root_identity(), root_ty, projections))
    }

    fn needs_cleanup(&mut self, ty: TypeId) -> Result<bool, BodyCheckInternalError> {
        if matches!(self.types.get(ty), Some(TypeKind::Borrow { .. })) {
            return Ok(false);
        }
        self.copyabilities
            .classify_with_proofs(self.graph, self.types, ty, self.copy_proofs)
            .map(|copyability| copyability == Copyability::MoveOnly)
            .map_err(BodyCheckInternalError::Copyability)
    }

    fn destruction_effect(&mut self, ty: TypeId) -> Result<CleanupEffect, BodyCheckInternalError> {
        DestructionEffectResolver::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.closures,
            self.opaque_witnesses,
            self.copy_proofs,
        )
        .resolve(ty)
    }

    fn enum_residual_effect(
        &mut self,
        ty: TypeId,
        variant: nocter_model::VariantId,
        payload: &[nocter_model::ParameterId],
    ) -> Result<CleanupEffect, BodyCheckInternalError> {
        DestructionEffectResolver::new(
            self.graph,
            self.types,
            self.copyabilities,
            self.drops,
            self.closures,
            self.opaque_witnesses,
            self.copy_proofs,
        )
        .resolve_enum_residual(ty, variant, payload)
    }

    fn root_type(&self, root: PlaceRoot) -> Result<TypeId, BodyCheckInternalError> {
        match root {
            PlaceRoot::Parameter(parameter) => self
                .graph
                .declarations()
                .parameters()
                .get(parameter)
                .map(|parameter| parameter.ty())
                .ok_or(BodyCheckInternalError::CleanupPlanning),
            PlaceRoot::Local(local) => self
                .body
                .locals()
                .get(local)
                .map(|local| local.ty())
                .ok_or(BodyCheckInternalError::CleanupPlanning),
            PlaceRoot::Capture(capture) => self
                .body
                .captures()
                .get(capture)
                .map(|capture| capture.ty())
                .ok_or(BodyCheckInternalError::CleanupPlanning),
            PlaceRoot::Value(value) => self
                .body
                .nodes()
                .get(value)
                .map(crate::CheckedNode::ty)
                .ok_or(BodyCheckInternalError::CleanupPlanning),
            PlaceRoot::Static(_) => Err(BodyCheckInternalError::CleanupPlanning),
        }
    }

    fn aggregate_children(
        &mut self,
        ty: TypeId,
    ) -> Result<Vec<(MoveProjection, TypeId)>, BodyCheckInternalError> {
        if let Some(TypeKind::Tuple(elements)) = self.types.get(ty) {
            return Ok(elements
                .iter()
                .enumerate()
                .map(|(index, ty)| (MoveProjection::TupleElement(index), ty))
                .collect());
        }
        let (definition, arguments) = match self.types.get(ty).cloned() {
            Some(TypeKind::Nominal {
                definition,
                arguments,
            }) => (definition, arguments),
            Some(_) => return Err(BodyCheckInternalError::CleanupPlanning),
            None => return Err(BodyCheckInternalError::UnknownType(ty)),
        };
        if self.drops.get(definition).is_some() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let nominal = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .ok_or(BodyCheckInternalError::CleanupPlanning)?;
        let NominalShape::Struct { fields, .. } = nominal.shape() else {
            return Err(BodyCheckInternalError::CleanupPlanning);
        };
        if nominal.generic_parameters().len() != arguments.len() {
            return Err(BodyCheckInternalError::CleanupPlanning);
        }
        let mut substitution = TypeSubstitution::default();
        for (parameter, argument) in nominal
            .generic_parameters()
            .iter()
            .copied()
            .zip(arguments.iter().copied())
        {
            substitution.bind_generic(parameter, argument);
        }
        fields
            .iter()
            .copied()
            .map(|field| {
                let declaration = self
                    .graph
                    .declarations()
                    .fields()
                    .get(field)
                    .ok_or(BodyCheckInternalError::CleanupPlanning)?;
                let field_ty = substitution
                    .apply_type(self.types, declaration.ty())
                    .map_err(|_| BodyCheckInternalError::CleanupPlanning)?;
                Ok((MoveProjection::Field(field), field_ty))
            })
            .collect()
    }
}
