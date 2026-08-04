//! Typecheck facts that connect a direct callable-value invocation to the
//! concrete closure body selected during monomorphization.

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallableCallFact {
    pub(crate) signature: FunctionSignature,
    pub(crate) specialization: CallableCallSpecialization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallableCallSpecialization {
    pub(crate) callable_ty: TypeExpr,
    pub(crate) capability: crate::ast::CallableCapability,
    pub(crate) target_name: String,
    free_type_parameters: HashSet<String>,
}

pub(super) fn callable_call_fact(
    call: &CallExpr,
    contract: &crate::typecheck::callables::ResolvedCallableContract,
) -> Option<CallableCallFact> {
    let mut free_type_parameters = HashSet::new();
    let callable_ty = type_to_type_expr_allowing_parameters(
        &contract.callee_type,
        call.callee.span(),
        &mut free_type_parameters,
    )?;
    let target_name = callable_target_name(&callable_ty);
    Some(CallableCallFact {
        signature: contract.signature.clone(),
        specialization: CallableCallSpecialization {
            callable_ty,
            capability: contract.capability,
            target_name,
            free_type_parameters,
        },
    })
}

impl CallableCallSpecialization {
    pub(crate) fn with_context_substitutions(
        &self,
        context_substitutions: &HashMap<String, TypeExpr>,
    ) -> Option<Self> {
        let callable_ty = substitute_type_expr_parameters(&self.callable_ty, context_substitutions);
        if type_expr_contains_free_parameters(&callable_ty, &self.free_type_parameters) {
            return None;
        }
        Some(Self {
            target_name: callable_target_name(&callable_ty),
            callable_ty,
            capability: self.capability,
            free_type_parameters: HashSet::new(),
        })
    }

    pub(crate) fn receiver_mode(&self) -> MethodReceiverMode {
        match self.capability {
            crate::ast::CallableCapability::Readonly => MethodReceiverMode::ReadonlyBorrow,
            crate::ast::CallableCapability::Readwrite => MethodReceiverMode::ReadwriteBorrow,
            crate::ast::CallableCapability::Consuming => MethodReceiverMode::Owned,
        }
    }
}

fn callable_target_name(callable_ty: &TypeExpr) -> String {
    format!("{}.call", type_expr_display_lossy(callable_ty))
}
