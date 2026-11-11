//! Effect-only substitution of frozen callable bindings; no target search or type instantiation.
use super::{INVALID, IntegrityError, TypedQuery};
use crate::value_access::ExpressionKind;
use nocter_declarations::value_access::FunctionWitness;
use nocter_model::{AccessExpressionId, AccessParameterId, ValueCode, ValueFunctionOrigin};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Request {
    pub code: ValueCode,
    pub bindings: BTreeMap<AccessParameterId, FunctionWitness>,
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
            target, arguments, ..
        } = &typed.body().expressions().get(call).ok_or(INVALID)?.kind
        else {
            return Err(INVALID);
        };
        let origin = match *target {
            ValueFunctionOrigin::Parameter(parameter) => {
                self.bindings.get(&parameter).map(|witness| witness.origin)
            }
            origin => Some(origin),
        };
        let Some(code) = origin.and_then(super::target_code) else {
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
        Ok(match witness.origin {
            ValueFunctionOrigin::Declaration(_) | ValueFunctionOrigin::Closure(_) => Some(witness),
            ValueFunctionOrigin::Parameter(parameter) => {
                let Some(actual) = self.bindings.get(&parameter) else {
                    return Ok(None);
                };
                if !actual.invocation.can_supply(witness.invocation) {
                    return Err(INVALID);
                }
                Some(FunctionWitness {
                    origin: actual.origin,
                    invocation: witness.invocation,
                })
            }
            ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_) => None,
        })
    }
}
