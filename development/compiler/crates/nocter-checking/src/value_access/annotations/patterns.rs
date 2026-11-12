//! Resolve annotation leaves once while retaining recursive callable constraints as constraints.
use super::{Checker, Failure, FunctionPattern};
use nocter_declaration_lowering::access_declarations::{CallableTypeSyntax, TypeSyntax};
use nocter_declarations::value_access::{FunctionDataPattern, FunctionInput};

impl Checker<'_, '_> {
    pub(super) fn callable_annotation(
        &mut self,
        contract: &CallableTypeSyntax,
    ) -> Result<FunctionPattern, Failure> {
        Ok(FunctionPattern {
            invocation: contract.invocation,
            inputs: contract
                .inputs
                .iter()
                .map(|(operation, ty)| {
                    Ok(FunctionInput {
                        operation: *operation,
                        data: self.callable_data_pattern(ty)?,
                    })
                })
                .collect::<Result<_, Failure>>()?,
            result: self.callable_data_pattern(&contract.result)?,
        })
    }
    fn callable_data_pattern(&mut self, ty: &TypeSyntax) -> Result<FunctionDataPattern, Failure> {
        match ty {
            TypeSyntax::Callable(contract) => Ok(FunctionDataPattern::Function(Box::new(
                self.callable_annotation(contract)?,
            ))),
            data => self
                .annotation_data(data, false)
                .map(FunctionDataPattern::Data),
        }
    }
}
