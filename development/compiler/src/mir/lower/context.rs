//! Construction-only state shared by MIR statement and expression lowering.

use super::body_builder::ControlFlowBuilder;
use super::{BuildError, SemanticInputs};
use crate::mir::model::BasicBlock;
use crate::mir::{Local, LocalId, LoopRegion, ProjectionPath, Scope, ScopeId};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) struct LoweringContext<'a> {
    pub(super) semantic: SemanticInputs<'a>,
    pub(super) locals: Vec<Local>,
    pub(super) locals_by_symbol: HashMap<LocalSymbolId, LocalId>,
    pub(super) projections: Vec<ProjectionPath>,
    pub(super) control_flow: ControlFlowBuilder,
    pub(super) loop_regions: Vec<LoopRegion>,
    pub(super) scopes: Vec<Scope>,
}

pub(super) struct LoweredBodyParts {
    pub(super) locals: Vec<Local>,
    pub(super) projections: Vec<ProjectionPath>,
    pub(super) blocks: Vec<BasicBlock>,
    pub(super) loop_regions: Vec<LoopRegion>,
    pub(super) scopes: Vec<Scope>,
}

impl<'a> LoweringContext<'a> {
    pub(super) fn new(
        semantic: SemanticInputs<'a>,
        locals: Vec<Local>,
        locals_by_symbol: HashMap<LocalSymbolId, LocalId>,
        root_scope: ScopeId,
        root: Scope,
    ) -> Self {
        debug_assert_eq!(root_scope.index(), 0);
        Self {
            semantic,
            locals,
            locals_by_symbol,
            projections: Vec::new(),
            control_flow: ControlFlowBuilder::new(root_scope),
            loop_regions: Vec::new(),
            scopes: vec![root],
        }
    }

    pub(super) fn child_scope(
        &mut self,
        parent: ScopeId,
        span: crate::source::ByteSpan,
    ) -> ScopeId {
        let scope = ScopeId::from_index(self.scopes.len());
        self.scopes.push(Scope::child(parent, span));
        scope
    }

    pub(super) fn finish(self) -> Result<LoweredBodyParts, BuildError> {
        Ok(LoweredBodyParts {
            locals: self.locals,
            projections: self.projections,
            blocks: self.control_flow.finish()?,
            loop_regions: self.loop_regions,
            scopes: self.scopes,
        })
    }
}
