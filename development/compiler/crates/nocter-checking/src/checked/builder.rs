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
    locals: ArenaBuilder<LocalBindingId, CheckedLocal>,
    captures: ArenaBuilder<CaptureId, CheckedCapture>,
    places: ArenaBuilder<PlaceId, CheckedPlace>,
    loops: ArenaBuilder<LoopId, LoopSlot>,
    nodes: ArenaBuilder<BodyNodeId, CheckedNode>,
}

impl CheckedBodyBuilder {
    #[must_use]
    pub(crate) fn new(names: &ResolvedBodyNames) -> Self {
        Self {
            scopes: names.scopes().clone(),
            local_declarations: names.locals().clone(),
            capture_declarations: names.captures().clone(),
            locals: ArenaBuilder::new(),
            captures: ArenaBuilder::new(),
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
        let actual = self.locals.insert(CheckedLocal::new(declaration, ty));
        if actual != expected {
            return Err(BuildCheckedBodyError::NonCanonicalLocal { expected, actual });
        }
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
        self.locals.get(local).map(|local| local.ty())
    }

    pub(crate) fn node_type(&self, node: BodyNodeId) -> Option<TypeId> {
        self.nodes.get(node).map(CheckedNode::ty)
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

    pub(crate) fn finish(self, root: BodyNodeId) -> Result<CheckedBody, BuildCheckedBodyError> {
        if self.locals.len() != self.local_declarations.len() {
            return Err(BuildCheckedBodyError::IncompleteLocals {
                expected: self.local_declarations.len(),
                actual: self.locals.len(),
            });
        }
        if self.captures.len() != self.capture_declarations.len() {
            return Err(BuildCheckedBodyError::IncompleteCaptures {
                expected: self.capture_declarations.len(),
                actual: self.captures.len(),
            });
        }
        let loops = self.loops.try_finish_with(|loop_, slot| match slot {
            LoopSlot::Reserved => Err(BuildCheckedBodyError::IncompleteLoop(loop_)),
            LoopSlot::Defined(definition) => Ok(definition),
        })?;
        let nodes = self.nodes.finish();
        let mut cleanup_schedules =
            ArenaBuilder::<BodyNodeId, Option<super::CleanupSchedule>>::new();
        for _ in 0..nodes.len() {
            cleanup_schedules.insert(None);
        }
        Ok(CheckedBody::new(
            CheckedBodyDomains {
                scopes: self.scopes,
                locals: self.locals.finish(),
                captures: self.captures.finish(),
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
    NonCanonicalLocal {
        expected: LocalBindingId,
        actual: LocalBindingId,
    },
    IncompleteLocals {
        expected: usize,
        actual: usize,
    },
    IncompleteCaptures {
        expected: usize,
        actual: usize,
    },
    UnknownLoop(LoopId),
    DuplicateLoop(LoopId),
    IncompleteLoop(LoopId),
    InvalidCleanupCount {
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for BuildCheckedBodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid checked-body construction: {self:?}")
    }
}

impl std::error::Error for BuildCheckedBodyError {}
