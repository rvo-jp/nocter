use nocter_model::{
    Arena, BodyNodeId, BodyScopeId, CaptureId, LocalBindingId, LoopId, PlaceId, TypeId,
};

use crate::{BodyScope, Capture, LocalBinding};

use super::{CheckedLoop, CheckedNode, CheckedPlace};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedLocal {
    declaration: LocalBinding,
    ty: TypeId,
}

impl CheckedLocal {
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
#[derive(Debug)]
pub struct CheckedBody {
    scopes: Arena<BodyScopeId, BodyScope>,
    locals: Arena<LocalBindingId, CheckedLocal>,
    captures: Arena<CaptureId, CheckedCapture>,
    places: Arena<PlaceId, CheckedPlace>,
    loops: Arena<LoopId, CheckedLoop>,
    nodes: Arena<BodyNodeId, CheckedNode>,
    root: BodyNodeId,
}

impl CheckedBody {
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
    pub const fn root(&self) -> BodyNodeId {
        self.root
    }
}
