//! Construction-only state shared by MIR statement and expression lowering.

use super::body_builder::ControlFlowBuilder;
use super::{BuildError, SemanticInputs};
use crate::mir::model::BasicBlock;
use crate::mir::{
    AllocationContextOverride, AllocationRegion, DropPlan, Loan, Local, LocalId, LoopRegion,
    ProjectionPath, Scope, ScopeId,
};
use crate::resolve::LocalSymbolId;
use std::collections::HashMap;

pub(super) struct LoweringContext<'a> {
    pub(super) semantic: SemanticInputs<'a>,
    pub(super) outcome_contract: Option<crate::mir::OutcomeContract>,
    pub(super) locals: Vec<Local>,
    pub(super) places_by_symbol: HashMap<LocalSymbolId, crate::mir::Place>,
    pub(super) projections: Vec<ProjectionPath>,
    pub(super) drop_plans: Vec<DropPlan>,
    pub(super) control_flow: ControlFlowBuilder,
    pub(super) loop_regions: Vec<LoopRegion>,
    pub(super) loans: Vec<Loan>,
    pub(super) allocation_regions: Vec<AllocationRegion>,
    pub(super) allocation_overrides: Vec<AllocationContextOverride>,
    pub(super) scopes: Vec<Scope>,
}

pub(super) struct LoweredBodyParts {
    pub(super) locals: Vec<Local>,
    pub(super) projections: Vec<ProjectionPath>,
    pub(super) drop_plans: Vec<DropPlan>,
    pub(super) blocks: Vec<BasicBlock>,
    pub(super) loop_regions: Vec<LoopRegion>,
    pub(super) loans: Vec<Loan>,
    pub(super) allocation_regions: Vec<AllocationRegion>,
    pub(super) allocation_overrides: Vec<AllocationContextOverride>,
    pub(super) scopes: Vec<Scope>,
}

impl<'a> LoweringContext<'a> {
    pub(super) fn return_local(&self) -> LocalId {
        LocalId::from_index(0)
    }

    pub(super) fn new(
        semantic: SemanticInputs<'a>,
        outcome_contract: Option<crate::mir::OutcomeContract>,
        locals: Vec<Local>,
        places_by_symbol: HashMap<LocalSymbolId, crate::mir::Place>,
        drop_plans: Vec<DropPlan>,
        projections: Vec<ProjectionPath>,
        root_scope: ScopeId,
        root: Scope,
    ) -> Self {
        debug_assert_eq!(root_scope.index(), 0);
        Self {
            semantic,
            outcome_contract,
            locals,
            places_by_symbol,
            projections,
            drop_plans,
            control_flow: ControlFlowBuilder::new(root_scope),
            loop_regions: Vec::new(),
            loans: Vec::new(),
            allocation_regions: Vec::new(),
            allocation_overrides: Vec::new(),
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

    pub(super) fn aggregate_temporary(
        &mut self,
        ty: crate::semantic::TyId,
        origin: crate::mir::LocalOrigin,
        scope: ScopeId,
    ) -> Result<LocalId, BuildError> {
        let type_expr = self
            .semantic
            .typed_hir
            .type_expr_by_id(ty)
            .ok_or(BuildError::MissingTypedExpression)?;
        let ownership = if crate::typecheck::type_expr_is_copy(type_expr, self.semantic.resolved)
            == Some(true)
        {
            crate::mir::OwnershipKind::Copy
        } else {
            crate::mir::OwnershipKind::Move
        };
        let mut local = Local::aggregate(
            ty,
            ownership,
            crate::mir::LocalStorage::Local,
            origin,
            scope,
        );
        if ownership == crate::mir::OwnershipKind::Move {
            local.drop_plan = Some(
                super::super::drop_plans::build(
                    type_expr,
                    self.semantic.resolved,
                    self.semantic.resolved_sources,
                    self.semantic.typed_hir,
                    &mut self.drop_plans,
                )
                .ok_or(BuildError::UnsupportedClaimedExpression)?,
            );
        }
        let id = LocalId::from_index(self.locals.len());
        self.locals.push(local);
        Ok(id)
    }

    pub(super) fn local_for_type(
        &mut self,
        ty: crate::semantic::TyId,
        origin: crate::mir::LocalOrigin,
        scope: ScopeId,
    ) -> Result<LocalId, BuildError> {
        let representation = super::coverage::value_representation(ty, self.semantic)
            .ok_or(BuildError::MissingTypedExpression)?;
        if representation == crate::mir::ValueRepresentation::Aggregate {
            return self.aggregate_temporary(ty, origin, scope);
        }
        let local = match representation {
            crate::mir::ValueRepresentation::Unit => {
                Local::unit(ty, crate::mir::LocalStorage::Local, origin, scope)
            }
            crate::mir::ValueRepresentation::Scalar(scalar) => {
                Local::scalar(ty, scalar, crate::mir::LocalStorage::Local, origin, scope)
            }
            crate::mir::ValueRepresentation::View(kind) => {
                Local::view(ty, kind, crate::mir::LocalStorage::Local, origin, scope)
            }
            crate::mir::ValueRepresentation::Borrow => {
                let readwrite = self
                    .semantic
                    .typed_hir
                    .type_expr_by_id(ty)
                    .and_then(|ty| {
                        crate::typecheck::type_expr_borrow_readwrite(ty, self.semantic.resolved)
                    })
                    .ok_or(BuildError::MissingTypedExpression)?;
                Local::borrow(
                    ty,
                    readwrite,
                    crate::mir::LocalStorage::Local,
                    origin,
                    scope,
                )
            }
            crate::mir::ValueRepresentation::Error => {
                Local::error(ty, crate::mir::LocalStorage::Local, origin, scope)
            }
            crate::mir::ValueRepresentation::Aggregate => unreachable!(),
        };
        let id = LocalId::from_index(self.locals.len());
        self.locals.push(local);
        Ok(id)
    }

    pub(super) fn finish(self) -> Result<LoweredBodyParts, BuildError> {
        Ok(LoweredBodyParts {
            locals: self.locals,
            projections: self.projections,
            drop_plans: self.drop_plans,
            blocks: self.control_flow.finish()?,
            loop_regions: self.loop_regions,
            loans: self.loans,
            allocation_regions: self.allocation_regions,
            allocation_overrides: self.allocation_overrides,
            scopes: self.scopes,
        })
    }
}
