//! One-step substitution of frozen callable views, independent of executable admission.
use super::{FunctionMismatch, FunctionWitness};
use nocter_model::ValueFunctionOrigin;
#[cfg(test)]
mod tests;

impl FunctionWitness {
    /// Substitute one frozen formal binding, preserving the selected invocation view. Missing bindings
    /// and direct result/application terms return `None`. A bound symbolic term is not followed
    /// transitively. Callers retain domain, binding coverage, concreteness and admission checks.
    /// # Errors
    /// Rejects a supplied view that cannot provide the already selected invocation.
    pub fn substitute(
        self,
        binding: impl FnOnce(nocter_model::AccessParameterId) -> Option<Self>,
    ) -> Result<Option<Self>, FunctionMismatch> {
        match self.origin {
            ValueFunctionOrigin::Declaration(_) | ValueFunctionOrigin::Closure(_) => Ok(Some(self)),
            ValueFunctionOrigin::Parameter(parameter) => {
                let Some(actual) = binding(parameter) else {
                    return Ok(None);
                };
                if !actual.invocation.can_supply(self.invocation) {
                    return Err(FunctionMismatch::Invocation {
                        actual: actual.invocation,
                        expected: self.invocation,
                    });
                }
                Ok(Some(Self {
                    origin: actual.origin,
                    invocation: self.invocation,
                }))
            }
            ValueFunctionOrigin::Result(_) | ValueFunctionOrigin::Application(_) => Ok(None),
        }
    }
}
