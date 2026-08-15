//! Cached projection from checked `TyId` values to native ABI contracts.
//!
//! Machine-IR lowering must not independently resolve the same authored type
//! in aggregate, outcome, drop, and call paths.  This body-local projector is
//! the single gateway from the checked type arena into ABI layout.

use super::invalid_mir_diagnostics;
use crate::diagnostics::Diagnostic;
use crate::semantic::TyId;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(super) struct OutcomeProjection {
    pub(super) shape: crate::outcomes::OutcomeShape,
    pub(super) storage: crate::outcomes::storage::OutcomeStorageLayout,
    pub(super) payload_type: crate::ir::Type,
}

pub(super) struct TypeProjection<'a> {
    typed_hir: &'a crate::typecheck::TypedHir,
    resolved: &'a crate::resolve::ResolveOutput,
    resolved_sources: &'a crate::resolve::ResolvedSources<'a>,
    contracts: RefCell<HashMap<TyId, crate::abi::AbiTypeContract>>,
    outcomes: RefCell<HashMap<TyId, OutcomeProjection>>,
}

impl<'a> TypeProjection<'a> {
    pub(super) fn new(
        typed_hir: &'a crate::typecheck::TypedHir,
        resolved: &'a crate::resolve::ResolveOutput,
        resolved_sources: &'a crate::resolve::ResolvedSources<'a>,
    ) -> Self {
        Self {
            typed_hir,
            resolved,
            resolved_sources,
            contracts: RefCell::new(HashMap::new()),
            outcomes: RefCell::new(HashMap::new()),
        }
    }

    pub(super) fn contract(
        &self,
        ty: TyId,
    ) -> Result<crate::abi::AbiTypeContract, Vec<Diagnostic>> {
        if let Some(contract) = self.contracts.borrow().get(&ty) {
            return Ok(contract.clone());
        }
        let type_expr = self
            .typed_hir
            .type_expr_by_id(ty)
            .ok_or_else(|| invalid_mir_diagnostics("checked MIR type is missing"))?;
        let contract = self.contract_for_type_expr(type_expr)?;
        self.contracts.borrow_mut().insert(ty, contract.clone());
        Ok(contract)
    }

    pub(super) fn abi_value(&self, ty: TyId) -> Result<crate::abi::AbiValue, Vec<Diagnostic>> {
        match self.contract(ty)? {
            crate::abi::AbiTypeContract::Value(value) => Ok(value),
            contract => Err(invalid_mir_diagnostics(format!(
                "checked MIR type has no value ABI: {contract:?}"
            ))),
        }
    }

    pub(super) fn contract_for_type_expr(
        &self,
        type_expr: &crate::ast::TypeExpr,
    ) -> Result<crate::abi::AbiTypeContract, Vec<Diagnostic>> {
        crate::abi::abi_type_contract_from_type_expr_with_resolver(
            type_expr,
            self.resolved,
            |source| self.resolved_sources.get(&source).copied(),
        )
        .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))
    }

    pub(super) fn abi_value_for_type_expr(
        &self,
        type_expr: &crate::ast::TypeExpr,
    ) -> Result<crate::abi::AbiValue, Vec<Diagnostic>> {
        match self.contract_for_type_expr(type_expr)? {
            crate::abi::AbiTypeContract::Value(value) => Ok(value),
            contract => Err(invalid_mir_diagnostics(format!(
                "checked type has no value ABI: {contract:?}"
            ))),
        }
    }

    pub(super) fn outcome(&self, ty: TyId) -> Result<OutcomeProjection, Vec<Diagnostic>> {
        if let Some(outcome) = self.outcomes.borrow().get(&ty) {
            return Ok(outcome.clone());
        }
        let type_expr = self
            .typed_hir
            .type_expr_by_id(ty)
            .ok_or_else(|| invalid_mir_diagnostics("stored outcome type is missing"))?;
        let shape =
            crate::outcomes::outcome_shape_with_resolver(type_expr, self.resolved, |source| {
                self.resolved_sources.get(&source).copied()
            });
        let payload = match self.contract_for_type_expr(&shape.payload)? {
            crate::abi::AbiTypeContract::Value(value) => value,
            _ => {
                return Err(invalid_mir_diagnostics(
                    "stored outcome payload has no value ABI",
                ));
            }
        };
        let storage = shape
            .storage_layout(payload.layout)
            .ok_or_else(|| invalid_mir_diagnostics("stored outcome has unsupported layers"))?;
        let payload_type = super::super::types::return_type_from_type_expr_with_resolver(
            &shape.payload,
            self.resolved,
            |source| self.resolved_sources.get(&source).copied(),
        )
        .ok_or_else(|| invalid_mir_diagnostics("stored outcome payload is unsupported"))?;
        let outcome = OutcomeProjection {
            shape,
            storage,
            payload_type,
        };
        self.outcomes.borrow_mut().insert(ty, outcome.clone());
        Ok(outcome)
    }
}
