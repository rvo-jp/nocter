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

/// Source-neutral checked-body graph retained between body checking and current-source projection.
///
/// Semantic identities in this graph belong to the body transaction that produced it. The matching
/// body type and closure recipes must therefore rebind it before it becomes a [`CheckedBody`].
#[derive(Clone, Debug)]
pub(crate) struct CheckedBodyRecipe {
    local_types: Arena<LocalBindingId, TypeId>,
    capture_types: Arena<CaptureId, TypeId>,
    places: Arena<PlaceId, CheckedPlace>,
    loops: Arena<LoopId, CheckedLoop>,
    nodes: Arena<BodyNodeId, CheckedNode>,
    cleanups: CleanupTable,
    root: BodyNodeId,
}

pub(super) struct CheckedBodyDomains {
    pub(super) locals: Arena<LocalBindingId, CheckedLocal>,
    pub(super) captures: Arena<CaptureId, CheckedCapture>,
    pub(super) places: Arena<PlaceId, CheckedPlace>,
    pub(super) loops: Arena<LoopId, CheckedLoop>,
    pub(super) nodes: Arena<BodyNodeId, CheckedNode>,
}

impl CheckedBody {
    pub(crate) fn rebind(
        recipe: CheckedBodyRecipe,
        names: &crate::ResolvedBodyNames,
        source: SourceId,
        semantics: &super::CheckedSemanticRebinder<'_>,
    ) -> Result<Self, super::CheckedSemanticRebindError> {
        let locals = names.locals().try_map(|local, declaration| {
            let ty = recipe
                .local_types
                .get(local)
                .copied()
                .ok_or(super::CheckedSemanticRebindError::MissingLocal(local))?;
            Ok::<_, super::CheckedSemanticRebindError>(CheckedLocal::new(
                *declaration,
                semantics.ty(ty)?,
            ))
        })?;
        if recipe.local_types.len() != locals.len() {
            return Err(super::CheckedSemanticRebindError::LocalDomainMismatch);
        }
        let captures = names.captures().try_map(|capture, declaration| {
            let ty = recipe
                .capture_types
                .get(capture)
                .copied()
                .ok_or(super::CheckedSemanticRebindError::MissingCapture(capture))?;
            Ok::<_, super::CheckedSemanticRebindError>(CheckedCapture::new(
                *declaration,
                semantics.ty(ty)?,
            ))
        })?;
        if recipe.capture_types.len() != captures.len() {
            return Err(super::CheckedSemanticRebindError::CaptureDomainMismatch);
        }
        let places = recipe.places.try_map(|_, place| {
            let mut place = place.clone();
            place.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(place)
        })?;
        let loops = recipe.loops.try_map(|_, loop_| {
            let mut loop_ = loop_.clone();
            loop_.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(loop_)
        })?;
        let nodes = recipe.nodes.try_map(|_, node| {
            let mut node = node.clone();
            node.rebind(semantics)?;
            Ok::<_, super::CheckedSemanticRebindError>(node)
        })?;
        Ok(Self {
            source,
            scopes: names.scopes().clone(),
            locals,
            captures,
            places,
            loops,
            nodes,
            cleanups: recipe.cleanups,
            root: recipe.root,
        })
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

impl CheckedBodyRecipe {
    pub(super) fn new(
        domains: CheckedBodyDomains,
        cleanups: CleanupTable,
        root: BodyNodeId,
    ) -> Self {
        let local_types = domains.locals.map(|_, local| local.ty());
        let capture_types = domains.captures.map(|_, capture| capture.ty());
        Self {
            local_types,
            capture_types,
            places: domains.places,
            loops: domains.loops,
            nodes: domains.nodes,
            cleanups,
            root,
        }
    }
}
