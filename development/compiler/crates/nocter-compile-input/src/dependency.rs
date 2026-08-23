use nocter_syntax::NodeId;

use crate::ModuleIdentity;

/// One exact physical source selected for an authored `include`.
///
/// The target is a canonical source path rather than a module identity. Consumers can validate
/// source ownership and direct visibility without acquiring filesystem probing authority.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IncludeResolutionInput {
    declaration: NodeId,
    target_source: Box<str>,
}

impl IncludeResolutionInput {
    #[must_use]
    pub fn new(declaration: NodeId, target_source: impl Into<Box<str>>) -> Self {
        Self {
            declaration,
            target_source: target_source.into(),
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn target_source(&self) -> &str {
        &self.target_source
    }
}

/// One exact directory module selected for an authored `use`.
///
/// A `use` cannot carry a source target. This closed shape prevents lowering from preserving or
/// recreating the removed source-or-module import decision.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct UseResolutionInput {
    declaration: NodeId,
    target_module: ModuleIdentity,
}

impl UseResolutionInput {
    #[must_use]
    pub const fn new(declaration: NodeId, target_module: ModuleIdentity) -> Self {
        Self {
            declaration,
            target_module,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> NodeId {
        self.declaration
    }

    #[must_use]
    pub const fn target_module(&self) -> &ModuleIdentity {
        &self.target_module
    }
}
