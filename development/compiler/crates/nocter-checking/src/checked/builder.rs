use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, BodyNodeId, BodyScopeId, CaptureId, LocalBindingId, LoopId, PlaceId,
    TypeId,
};

use crate::{BodyScope, Capture, LocalBinding, ResolvedBodyNames};

use super::body::CheckedBodyDomains;
use super::{
    CheckedBody, CheckedCapture, CheckedLocal, CheckedLoop, CheckedNode, CheckedOperation,
    CheckedPlace, CleanupTable, PlaceAccess, PlaceProjection, PlaceRoot,
};

/// Sole mutable construction path for one checked body.
pub(crate) struct CheckedBodyBuilder {
    scopes: Arena<BodyScopeId, BodyScope>,
    local_declarations: Arena<LocalBindingId, LocalBinding>,
    capture_declarations: Arena<CaptureId, Capture>,
    locals: ArenaBuilder<LocalBindingId, Option<CheckedLocal>>,
    captures: ArenaBuilder<CaptureId, Option<CheckedCapture>>,
    places: ArenaBuilder<PlaceId, CheckedPlace>,
    loops: ArenaBuilder<LoopId, LoopSlot>,
    nodes: ArenaBuilder<BodyNodeId, CheckedNode>,
}

impl CheckedBodyBuilder {
    #[must_use]
    pub(crate) fn new(names: &ResolvedBodyNames) -> Self {
        let mut locals = ArenaBuilder::new();
        for _ in 0..names.locals().len() {
            locals.insert(None);
        }
        let mut captures = ArenaBuilder::new();
        for _ in 0..names.captures().len() {
            captures.insert(None);
        }
        Self {
            scopes: names.scopes().clone(),
            local_declarations: names.locals().clone(),
            capture_declarations: names.captures().clone(),
            locals,
            captures,
            places: ArenaBuilder::new(),
            loops: ArenaBuilder::new(),
            nodes: ArenaBuilder::new(),
        }
    }

    pub(crate) fn define_local(
        &mut self,
        expected: LocalBindingId,
        ty: TypeId,
    ) -> Result<(), BuildCheckedBodyError> {
        let declaration = self
            .local_declarations
            .get(expected)
            .copied()
            .ok_or(BuildCheckedBodyError::UnknownLocal(expected))?;
        let slot = self
            .locals
            .get_mut(expected)
            .ok_or(BuildCheckedBodyError::UnknownLocal(expected))?;
        if slot.is_some() {
            return Err(BuildCheckedBodyError::DuplicateLocal(expected));
        }
        *slot = Some(CheckedLocal::new(declaration, ty));
        Ok(())
    }

    pub(crate) fn define_capture(
        &mut self,
        expected: CaptureId,
        ty: TypeId,
    ) -> Result<(), BuildCheckedBodyError> {
        let declaration = self
            .capture_declarations
            .get(expected)
            .copied()
            .ok_or(BuildCheckedBodyError::UnknownCapture(expected))?;
        let slot = self
            .captures
            .get_mut(expected)
            .ok_or(BuildCheckedBodyError::UnknownCapture(expected))?;
        if slot.is_some() {
            return Err(BuildCheckedBodyError::DuplicateCapture(expected));
        }
        *slot = Some(CheckedCapture::new(declaration, ty));
        Ok(())
    }

    pub(crate) fn add_place(
        &mut self,
        root: PlaceRoot,
        projections: impl Into<Box<[PlaceProjection]>>,
        ty: TypeId,
        access: PlaceAccess,
        writable: bool,
    ) -> PlaceId {
        self.places
            .insert(CheckedPlace::new(root, projections, ty, access, writable))
    }

    pub(crate) fn place(&self, place: PlaceId) -> Option<&CheckedPlace> {
        self.places.get(place)
    }

    pub(crate) fn add_node(&mut self, ty: TypeId, operation: CheckedOperation) -> BodyNodeId {
        self.nodes.insert(CheckedNode::new(ty, operation))
    }

    pub(crate) fn local_type(&self, local: LocalBindingId) -> Option<TypeId> {
        self.locals
            .get(local)
            .and_then(|local| local.as_ref())
            .map(|local| local.ty())
    }

    pub(crate) fn capture_type(&self, capture: CaptureId) -> Option<TypeId> {
        self.captures
            .get(capture)
            .and_then(|capture| capture.as_ref())
            .map(|capture| capture.ty())
    }

    pub(crate) fn node_type(&self, node: BodyNodeId) -> Option<TypeId> {
        self.nodes.get(node).map(CheckedNode::ty)
    }

    pub(crate) fn node(&self, node: BodyNodeId) -> Option<&CheckedNode> {
        self.nodes.get(node)
    }

    pub(crate) fn replace_operation(
        &mut self,
        node: BodyNodeId,
        operation: CheckedOperation,
    ) -> Result<(), BuildCheckedBodyError> {
        let checked = self
            .nodes
            .get_mut(node)
            .ok_or(BuildCheckedBodyError::UnknownNode(node))?;
        checked.replace_operation(operation);
        Ok(())
    }

    pub(crate) fn reserve_loop(&mut self) -> LoopId {
        self.loops.insert(LoopSlot::Reserved)
    }

    pub(crate) fn define_loop(
        &mut self,
        loop_: LoopId,
        definition: CheckedLoop,
    ) -> Result<(), BuildCheckedBodyError> {
        let slot = self
            .loops
            .get_mut(loop_)
            .ok_or(BuildCheckedBodyError::UnknownLoop(loop_))?;
        if matches!(slot, LoopSlot::Defined(_)) {
            return Err(BuildCheckedBodyError::DuplicateLoop(loop_));
        }
        *slot = LoopSlot::Defined(definition);
        Ok(())
    }

    pub(crate) fn loop_definition(&self, loop_: LoopId) -> Option<&CheckedLoop> {
        match self.loops.get(loop_) {
            Some(LoopSlot::Defined(definition)) => Some(definition),
            Some(LoopSlot::Reserved) | None => None,
        }
    }

    pub(crate) fn finish(self, root: BodyNodeId) -> Result<CheckedBody, BuildCheckedBodyError> {
        let locals = self.locals.try_finish_with(|local, slot| {
            slot.ok_or(BuildCheckedBodyError::IncompleteLocal(local))
        })?;
        let captures = self.captures.try_finish_with(|capture, slot| {
            slot.ok_or(BuildCheckedBodyError::IncompleteCapture(capture))
        })?;
        let loops = self.loops.try_finish_with(|loop_, slot| match slot {
            LoopSlot::Reserved => Err(BuildCheckedBodyError::IncompleteLoop(loop_)),
            LoopSlot::Defined(definition) => Ok(definition),
        })?;
        let nodes = self.nodes.finish();
        let mut cleanup_schedules =
            ArenaBuilder::<BodyNodeId, Box<[super::CleanupSchedule]>>::new();
        for _ in 0..nodes.len() {
            cleanup_schedules.insert(Box::new([]));
        }
        Ok(CheckedBody::new(
            CheckedBodyDomains {
                scopes: self.scopes,
                locals,
                captures,
                places: self.places.finish(),
                loops,
                nodes,
            },
            CleanupTable::new(cleanup_schedules.finish()),
            root,
        ))
    }
}

#[derive(Debug)]
enum LoopSlot {
    Reserved,
    Defined(CheckedLoop),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildCheckedBodyError {
    UnknownLocal(LocalBindingId),
    DuplicateLocal(LocalBindingId),
    IncompleteLocal(LocalBindingId),
    UnknownCapture(CaptureId),
    DuplicateCapture(CaptureId),
    IncompleteCapture(CaptureId),
    UnknownLoop(LoopId),
    DuplicateLoop(LoopId),
    IncompleteLoop(LoopId),
    UnknownNode(BodyNodeId),
    InvalidCleanupCount { expected: usize, actual: usize },
}

impl fmt::Display for BuildCheckedBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid checked-body construction: {self:?}")
    }
}

impl std::error::Error for BuildCheckedBodyError {}
