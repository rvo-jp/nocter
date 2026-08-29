use nocter_model::{
    Arena, BodyNodeId, BodyScopeId, CaptureId, LocalBindingId, LoopId, PlaceId, TypeId,
};
use nocter_source::SourceId;

use crate::{BodyScope, Capture, LocalBinding};

use super::{CheckedLoop, CheckedNode, CheckedPlace, CleanupTable};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedLocal {
    declaration: LocalBinding,
    ty: TypeId,
}

impl CheckedLocal {
    pub(super) const fn new(declaration: LocalBinding, ty: TypeId) -> Self {
        Self { declaration, ty }
    }

    #[must_use]
    pub const fn declaration(self) -> LocalBinding {
        self.declaration
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCapture {
    declaration: Capture,
    ty: TypeId,
}

impl CheckedCapture {
    pub(super) const fn new(declaration: Capture, ty: TypeId) -> Self {
        Self { declaration, ty }
    }

    #[must_use]
    pub const fn declaration(self) -> Capture {
        self.declaration
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One complete typed body. Syntax-backed name uses have been consumed before construction.
#[derive(Clone, Debug)]
pub struct CheckedBody {
    source: SourceId,
    scopes: Arena<BodyScopeId, BodyScope>,
    locals: Arena<LocalBindingId, CheckedLocal>,
    captures: Arena<CaptureId, CheckedCapture>,
    places: Arena<PlaceId, CheckedPlace>,
    loops: Arena<LoopId, CheckedLoop>,
    nodes: Arena<BodyNodeId, CheckedNode>,
    cleanups: CleanupTable,
    root: BodyNodeId,
}

pub(super) struct CheckedBodyDomains {
    pub(super) scopes: Arena<BodyScopeId, BodyScope>,
    pub(super) locals: Arena<LocalBindingId, CheckedLocal>,
    pub(super) captures: Arena<CaptureId, CheckedCapture>,
    pub(super) places: Arena<PlaceId, CheckedPlace>,
    pub(super) loops: Arena<LoopId, CheckedLoop>,
    pub(super) nodes: Arena<BodyNodeId, CheckedNode>,
}

impl CheckedBody {
    pub(crate) fn rebind(
        mut self,
        source: SourceId,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<Self, super::CheckedSemanticRebindError> {
        self.source = source;
        self.locals = self.locals.try_map(|_, local| {
            let mut local = *local;
            local.ty = semantics.ty(local.ty)?;
            Ok::<_, super::CheckedSemanticRebindError>(local)
        })?;
        self.captures = self.captures.try_map(|_, capture| {
            let mut capture = *capture;
            capture.ty = semantics.ty(capture.ty)?;
            Ok::<_, super::CheckedSemanticRebindError>(capture)
        })?;
        self.places = self.places.try_map(|_, place| {
            let mut place = place.clone();
            place.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(place)
        })?;
        self.loops = self.loops.try_map(|_, loop_| {
            let mut loop_ = loop_.clone();
            loop_.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(loop_)
        })?;
        self.nodes = self.nodes.try_map(|_, node| {
            let mut node = node.clone();
            node.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(node)
        })?;
        Ok(self)
    }
    pub(super) fn new(
        source: SourceId,
        domains: CheckedBodyDomains,
        cleanups: CleanupTable,
        root: BodyNodeId,
    ) -> Self {
        Self {
            source,
            scopes: domains.scopes,
            locals: domains.locals,
            captures: domains.captures,
            places: domains.places,
            loops: domains.loops,
            nodes: domains.nodes,
            cleanups,
            root,
        }
    }

    /// Returns the physical source under whose direct `see` visibility this body was checked.
    #[must_use]
    pub const fn source(&self) -> SourceId {
        self.source
    }

    #[must_use]
    pub const fn scopes(&self) -> &Arena<BodyScopeId, BodyScope> {
        &self.scopes
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<LocalBindingId, CheckedLocal> {
        &self.locals
    }

    #[must_use]
    pub const fn captures(&self) -> &Arena<CaptureId, CheckedCapture> {
        &self.captures
    }

    #[must_use]
    pub const fn places(&self) -> &Arena<PlaceId, CheckedPlace> {
        &self.places
    }

    #[must_use]
    pub const fn loops(&self) -> &Arena<LoopId, CheckedLoop> {
        &self.loops
    }

    #[must_use]
    pub const fn nodes(&self) -> &Arena<BodyNodeId, CheckedNode> {
        &self.nodes
    }

    #[must_use]
    pub const fn cleanups(&self) -> &CleanupTable {
        &self.cleanups
    }

    pub(crate) fn attach_cleanups(
        &mut self,
        cleanups: CleanupTable,
    ) -> Result<(), super::BuildCheckedBodyError> {
        if cleanups.len() != self.nodes.len() {
            return Err(super::BuildCheckedBodyError::InvalidCleanupCount {
                expected: self.nodes.len(),
                actual: cleanups.len(),
            });
        }
        self.cleanups = cleanups;
        Ok(())
    }

    #[must_use]
    pub const fn root(&self) -> BodyNodeId {
        self.root
    }
}
