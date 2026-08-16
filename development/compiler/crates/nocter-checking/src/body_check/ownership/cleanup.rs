use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::{BodyScopeId, TypeId, TypeKind, TypeStore};

use super::super::error::BodyCheckInternalError;
use crate::copyability::{Copyability, CopyabilityTable};
use crate::ownership::{InitializationState, MovePath, OwnershipState, initialized_body_roots};
use crate::type_relations::TypeSubstitution;
use crate::{
    BodySource, CheckedBody, CleanupAction, CleanupCondition, CleanupPath, CleanupTarget,
    DropTable, PlaceRoot,
};

pub(super) struct CleanupPlanner<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut TypeStore,
    copyabilities: &'program mut CopyabilityTable,
    drops: &'program DropTable,
    body: &'program CheckedBody,
    source: BodySource<'program>,
}

impl<'program> CleanupPlanner<'program> {
    pub(super) fn new(
        graph: &'program DeclarationGraph,
        types: &'program mut TypeStore,
        copyabilities: &'program mut CopyabilityTable,
        drops: &'program DropTable,
        body: &'program CheckedBody,
        source: BodySource<'program>,
    ) -> Self {
        Self {
            graph,
            types,
            copyabilities,
            drops,
            body,
            source,
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
            .filter(|(_, local)| local.declaration().scope() == scope)
            .map(|(local, checked)| (local, checked.ty()))
            .collect::<Vec<_>>();
        let mut actions = Vec::new();
        while let Some((local, ty)) = locals.pop() {
            let root = PlaceRoot::Local(local);
            self.plan_path(&MovePath::root(root), ty, state, &mut actions)?;
            state.forget_root(root);
        }
        Ok(actions)
    }

    pub(super) fn parameter_actions(
        &mut self,
        state: &mut OwnershipState,
    ) -> Result<Vec<CleanupAction>, BodyCheckInternalError> {
        let mut roots = initialized_body_roots(self.graph, self.source).ok_or(
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

    pub(super) fn value_action(
        &mut self,
        node: nocter_model::BodyNodeId,
        ty: TypeId,
    ) -> Result<Option<CleanupAction>, BodyCheckInternalError> {
        Ok(self.needs_cleanup(ty)?.then(|| {
            CleanupAction::new(CleanupTarget::Value { node, ty }, CleanupCondition::Always)
        }))
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
            CleanupTarget::Path(CleanupPath::new(path.root_identity(), path.fields(), ty)),
            CleanupCondition::Always,
        ))
    }

    fn plan_path(
        &mut self,
        path: &MovePath,
        ty: TypeId,
        state: &OwnershipState,
        actions: &mut Vec<CleanupAction>,
    ) -> Result<(), BodyCheckInternalError> {
        if state.has_descendant(path) {
            let fields = self.struct_fields(ty)?;
            for (field, field_ty) in fields.into_iter().rev() {
                self.plan_path(&path.field(field), field_ty, state, actions)?;
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
        actions.push(CleanupAction::new(
            CleanupTarget::Path(CleanupPath::new(path.root_identity(), path.fields(), ty)),
            condition,
        ));
        Ok(())
    }

    fn needs_cleanup(&mut self, ty: TypeId) -> Result<bool, BodyCheckInternalError> {
        if matches!(self.types.get(ty), Some(TypeKind::Borrow { .. })) {
            return Ok(false);
        }
        self.copyabilities
            .classify(self.graph, self.types, ty)
            .map(|copyability| copyability == Copyability::MoveOnly)
            .map_err(BodyCheckInternalError::Copyability)
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
        }
    }

    fn struct_fields(
        &mut self,
        ty: TypeId,
    ) -> Result<Vec<(nocter_model::FieldId, TypeId)>, BodyCheckInternalError> {
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
                Ok((field, field_ty))
            })
            .collect()
    }
}
