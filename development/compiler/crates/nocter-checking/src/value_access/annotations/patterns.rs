//! Resolve annotation leaves once while retaining recursive callable constraints as constraints.
use super::{Checker, Failure, FunctionPattern, Rule};
use nocter_declaration_lowering::access_declarations::{CallableTypeSyntax, TypeSyntax};
use nocter_declarations::value_access::{FunctionDataPattern, FunctionInput};

impl Checker<'_, '_> {
    pub(super) fn callable_annotation(
        &mut self,
        contract: &CallableTypeSyntax,
    ) -> Result<FunctionPattern, Failure> {
        self.callable_annotation_in(contract, false)
    }
    pub(super) fn callable_annotation_in(
        &mut self,
        contract: &CallableTypeSyntax,
        metadata_only: bool,
    ) -> Result<FunctionPattern, Failure> {
        if let Some((first, repeated)) = contract.repeated_input_name(self.syntax()) {
            let related = nocter_diagnostics::DiagnosticNote::new(
                "the first input binding with this name is here",
                crate::value_access::source_origin(
                    self.syntax(),
                    nocter_syntax::SyntaxOrigin::Token(first),
                )?,
            );
            return self.reject_message(
                Rule::ShadowedName,
                repeated,
                "duplicate callable input name".into(),
                vec![related],
            );
        }
        Ok(FunctionPattern {
            invocation: contract.invocation,
            inputs: contract
                .inputs
                .iter()
                .map(|input| {
                    let data = self.callable_data_pattern(&input.data, metadata_only)?;
                    if let Some(binding) = &input.binding
                        && !data.matches_input_binding(self.program(), &binding.shape)
                    {
                        return self.reject_message(
                            Rule::TypeMismatch,
                            binding.origin,
                            "parameter pattern does not match the declared tuple data".into(),
                            Vec::new(),
                        );
                    }
                    Ok(FunctionInput {
                        operation: input.operation,
                        data,
                    })
                })
                .collect::<Result<_, Failure>>()?,
            result: self.callable_data_pattern(&contract.result, metadata_only)?,
        })
    }
    fn callable_data_pattern(
        &mut self,
        ty: &TypeSyntax,
        metadata_only: bool,
    ) -> Result<FunctionDataPattern, Failure> {
        if !contains_callable(ty) {
            return self
                .annotation_data(ty, metadata_only)
                .map(FunctionDataPattern::Data);
        }
        Ok(match ty {
            TypeSyntax::Callable(contract) => FunctionDataPattern::Function(Box::new(
                self.callable_annotation_in(contract, metadata_only)?,
            )),
            TypeSyntax::Optional { payload, layers } => {
                let mut pattern = self.callable_data_pattern(payload, metadata_only)?;
                for _ in 0..layers.get() {
                    pattern = FunctionDataPattern::Optional(Box::new(pattern));
                }
                pattern
            }
            TypeSyntax::Fallible { payload, .. } => FunctionDataPattern::Fallible(Box::new(
                self.callable_data_pattern(payload, metadata_only)?,
            )),
            TypeSyntax::Array { element, length } => {
                let length = self.required_length(*length)?;
                FunctionDataPattern::Array {
                    element: Box::new(
                        self.callable_data_pattern(element, metadata_only || length == 0)?,
                    ),
                    length,
                }
            }
            TypeSyntax::Tuple(fields) => FunctionDataPattern::Tuple(
                fields
                    .iter()
                    .map(|field| self.callable_data_pattern(field, metadata_only))
                    .collect::<Result<_, _>>()?,
            ),
            TypeSyntax::Named(_) | TypeSyntax::Future { .. } => {
                unreachable!("a callable pattern must carry a represented callable")
            }
        })
    }
}

fn contains_callable(ty: &TypeSyntax) -> bool {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        match ty {
            TypeSyntax::Callable(_) => return true,
            TypeSyntax::Optional { payload, .. } | TypeSyntax::Fallible { payload, .. } => {
                pending.push(payload);
            }
            TypeSyntax::Array { element, .. } => pending.push(element),
            TypeSyntax::Tuple(fields) => pending.extend(fields),
            TypeSyntax::Named(_) | TypeSyntax::Future { .. } => {}
        }
    }
    false
}
