use std::collections::HashMap;

use nocter_declarations::ExportedEntity;
use nocter_model::{Arena, BodyId, BodyScopeId, CaptureId, LocalBindingId, ParameterId, Symbol};
use nocter_syntax::NodeId;
use nocter_syntax::SyntaxOrigin;

/// One exact value/name target selected during body lookup.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NameTarget {
    Parameter(ParameterId),
    Local(LocalBindingId),
    Capture(CaptureId),
    Exported(ExportedEntity),
}

impl NameTarget {
    pub(super) const fn is_callable_binding(self) -> bool {
        matches!(self, Self::Parameter(_) | Self::Local(_) | Self::Capture(_))
    }
}

/// The source construct that introduced a body-local binding.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocalBindingKind {
    Immutable,
    Mutable,
    PatternPayload,
    Loop,
    Region,
    Catch,
    ClosureParameter,
}

/// One syntax-independent body-local binding identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalBinding {
    name: Symbol,
    scope: BodyScopeId,
    kind: LocalBindingKind,
}

impl LocalBinding {
    pub(super) const fn new(name: Symbol, scope: BodyScopeId, kind: LocalBindingKind) -> Self {
        Self { name, scope, kind }
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn scope(self) -> BodyScopeId {
        self.scope
    }

    #[must_use]
    pub const fn kind(self) -> LocalBindingKind {
        self.kind
    }
}

/// Ownership/borrow mode authored for one explicit closure capture.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CaptureMode {
    Readonly,
    ReadWrite,
    Move,
}

/// One explicit closure-environment projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capture {
    name: Symbol,
    scope: BodyScopeId,
    source: NameTarget,
    mode: CaptureMode,
}

impl Capture {
    pub(super) const fn new(
        name: Symbol,
        scope: BodyScopeId,
        source: NameTarget,
        mode: CaptureMode,
    ) -> Self {
        Self {
            name,
            scope,
            source,
            mode,
        }
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn scope(self) -> BodyScopeId {
        self.scope
    }

    #[must_use]
    pub const fn source(self) -> NameTarget {
        self.source
    }

    #[must_use]
    pub const fn mode(self) -> CaptureMode {
        self.mode
    }
}

/// One lexical scope. Closure roots deliberately have no parent lookup edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeBinding {
    name: Symbol,
    target: NameTarget,
}

impl ScopeBinding {
    pub(super) const fn new(name: Symbol, target: NameTarget) -> Self {
        Self { name, target }
    }

    #[must_use]
    pub const fn name(self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn target(self) -> NameTarget {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodyScope {
    parent: Option<BodyScopeId>,
    bindings: Vec<ScopeBinding>,
}

impl BodyScope {
    pub(super) const fn new(parent: Option<BodyScopeId>) -> Self {
        Self {
            parent,
            bindings: Vec::new(),
        }
    }

    #[must_use]
    pub const fn parent(&self) -> Option<BodyScopeId> {
        self.parent
    }

    #[must_use]
    pub fn bindings(&self) -> &[ScopeBinding] {
        &self.bindings
    }

    pub(super) fn add_binding(&mut self, binding: ScopeBinding) {
        self.bindings.push(binding);
    }
}

/// One syntax-backed reference selected for later typed-node construction.
///
/// This is temporary checking input, not canonical checked-program state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedNameUse {
    origin: SyntaxOrigin,
    target: NameTarget,
}

impl ResolvedNameUse {
    pub(super) const fn new(origin: SyntaxOrigin, target: NameTarget) -> Self {
        Self { origin, target }
    }

    #[must_use]
    pub const fn origin(self) -> SyntaxOrigin {
        self.origin
    }

    #[must_use]
    pub const fn target(self) -> NameTarget {
        self.target
    }
}

/// Complete lexical-name result for one declaration body.
///
/// Syntax origins are consumed when typed body nodes are built. They never enter
/// `CheckedProgram`.
#[derive(Debug)]
pub struct ResolvedBodyNames {
    body: BodyId,
    scopes: Arena<BodyScopeId, BodyScope>,
    locals: Arena<LocalBindingId, LocalBinding>,
    captures: Arena<CaptureId, Capture>,
    local_origins: Arena<LocalBindingId, SyntaxOrigin>,
    capture_origins: Arena<CaptureId, SyntaxOrigin>,
    block_scopes: HashMap<NodeId, BodyScopeId>,
    uses: Box<[ResolvedNameUse]>,
}

pub(super) struct ResolvedBindingOrigins {
    pub(super) locals: Arena<LocalBindingId, SyntaxOrigin>,
    pub(super) captures: Arena<CaptureId, SyntaxOrigin>,
}

impl ResolvedBodyNames {
    pub(super) fn new(
        body: BodyId,
        scopes: Arena<BodyScopeId, BodyScope>,
        locals: Arena<LocalBindingId, LocalBinding>,
        captures: Arena<CaptureId, Capture>,
        origins: ResolvedBindingOrigins,
        block_scopes: HashMap<NodeId, BodyScopeId>,
        uses: impl Into<Box<[ResolvedNameUse]>>,
    ) -> Self {
        Self {
            body,
            scopes,
            locals,
            captures,
            local_origins: origins.locals,
            capture_origins: origins.captures,
            block_scopes,
            uses: uses.into(),
        }
    }

    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn scopes(&self) -> &Arena<BodyScopeId, BodyScope> {
        &self.scopes
    }

    #[must_use]
    pub const fn locals(&self) -> &Arena<LocalBindingId, LocalBinding> {
        &self.locals
    }

    #[must_use]
    pub const fn captures(&self) -> &Arena<CaptureId, Capture> {
        &self.captures
    }

    #[must_use]
    pub fn local_origin(&self, local: LocalBindingId) -> Option<SyntaxOrigin> {
        self.local_origins.get(local).copied()
    }

    #[must_use]
    pub fn capture_origin(&self, capture: CaptureId) -> Option<SyntaxOrigin> {
        self.capture_origins.get(capture).copied()
    }

    #[must_use]
    pub fn block_scope(&self, block: NodeId) -> Option<BodyScopeId> {
        self.block_scopes.get(&block).copied()
    }

    #[must_use]
    pub const fn uses(&self) -> &[ResolvedNameUse] {
        &self.uses
    }
}
