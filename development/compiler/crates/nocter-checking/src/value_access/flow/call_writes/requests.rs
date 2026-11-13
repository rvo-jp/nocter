//! Effect-only substitution of frozen callable bindings; no target search or type instantiation.
use super::{INVALID, IntegrityError, TypedQuery};
use crate::value_access::ExpressionKind;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{AccessExpressionId, ValueCode};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Request {
    pub code: ValueCode,
    pub bindings: BTreeMap<nocter_model::ValueParameter, FunctionWitness>,
}
impl Request {
    pub fn unbound(code: ValueCode) -> Self {
        Self {
            code,
            bindings: BTreeMap::new(),
        }
    }
    pub fn call(
        &self,
        typed: &TypedQuery,
        call: AccessExpressionId,
    ) -> Result<Option<Self>, IntegrityError> {
        let ExpressionKind::Call {
            target,
            arguments,
            kind,
        } = &typed.body().expressions().get(call).ok_or(INVALID)?.kind
        else {
            return Err(INVALID);
        };
        let selected = FunctionWitness {
            origin: *target,
            invocation: kind.invocation(arguments).ok_or(INVALID)?,
        };
        let Some(code) = self
            .resolve(selected)?
            .and_then(|witness| witness.origin.code())
        else {
            return Ok(None);
        };
        let mut bindings = BTreeMap::new();
        for argument in arguments {
            if let Some(binding) = &argument.binding
                && let Some(witness) = self.resolve(binding.witness)?
                && bindings.insert(binding.parameter, witness).is_some()
            {
                return Err(INVALID);
            }
        }
        Ok(Some(Self { code, bindings }))
    }
    pub fn resolve(
        &self,
        witness: FunctionWitness,
    ) -> Result<Option<FunctionWitness>, IntegrityError> {
        witness
            .substitute(|parameter| self.bindings.get(&parameter).copied())
            .map_err(|_| INVALID)
    }
}
