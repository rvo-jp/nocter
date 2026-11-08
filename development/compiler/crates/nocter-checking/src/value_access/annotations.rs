//! An annotation constrains data or a static callable witness; it never erases the witness.
use super::{Failure, Rule, checker::Checker};
use nocter_declarations::value_access::{FunctionConstraint, FunctionInput, FunctionMismatch};
use nocter_model::{AccessExpressionId, TypeId, ValueInvocation};
use nocter_syntax::{ContextualSpelling, Keyword, NodeId, NodeKind, TokenKind};

pub(super) fn invocation_label(invocation: ValueInvocation) -> &'static str {
    match invocation {
        ValueInvocation::Read => Keyword::Func.as_str(),
        ValueInvocation::Edit => ContextualSpelling::Mut.as_str(),
        ValueInvocation::Take => ContextualSpelling::Once.as_str(),
    }
}

pub(super) enum Annotation {
    Data(TypeId),
    Function(FunctionConstraint),
}
impl Annotation {
    pub fn data(&self) -> Option<TypeId> {
        match self {
            Self::Data(data) => Some(*data),
            Self::Function(_) => None,
        }
    }
}
impl Checker<'_, '_> {
    fn annotation_data(&self, node: NodeId) -> Result<TypeId, Failure> {
        let token = self.token(self.direct(node, NodeKind::NamedType)?)?;
        self.program()
            .type_named(self.text(token)?)
            .map_or_else(|| self.reject(Rule::UnknownType, token), Ok)
    }
    pub fn annotation(&self, node: NodeId) -> Result<Annotation, Failure> {
        let Ok(contract) = self.direct(node, NodeKind::CallableType) else {
            return self.annotation_data(node).map(Annotation::Data);
        };
        let token = self.token(contract)?;
        let invocation = match (
            token.kind(),
            ContextualSpelling::from_spelling(self.text(token)?),
        ) {
            (TokenKind::Keyword(Keyword::Func), _) => ValueInvocation::Read,
            (_, Some(ContextualSpelling::Mut)) => ValueInvocation::Edit,
            (_, Some(ContextualSpelling::Once)) => ValueInvocation::Take,
            _ => return Err(Failure::InconsistentSyntax),
        };
        let parameters = self.direct(contract, NodeKind::CallableParameters)?;
        let inputs = self
            .children(parameters)
            .into_iter()
            .map(|parameter| {
                let operation = nocter_declaration_lowering::access_declarations::input_operation(
                    self.syntax(),
                    parameter,
                )
                .map_err(|_| Failure::InconsistentSyntax)?;
                Ok(FunctionInput {
                    data: self.annotation_data(self.direct(parameter, NodeKind::Type)?)?,
                    operation,
                })
            })
            .collect::<Result<_, Failure>>()?;
        Ok(Annotation::Function(FunctionConstraint {
            invocation,
            inputs,
            result: self.annotation_data(self.direct(contract, NodeKind::Type)?)?,
        }))
    }
    pub fn function_constraint_data(
        &mut self,
        node: NodeId,
        value: AccessExpressionId,
        expected: &FunctionConstraint,
        alias: bool,
    ) -> Result<TypeId, Failure> {
        let Some(actual) = self.ty(value).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        let data = match self.program().constrain_function(actual.data(), expected) {
            Ok(data) => data,
            Err(FunctionMismatch::Integrity) => return Err(Failure::InconsistentSyntax),
            Err(mismatch) => {
                let message = match mismatch {
                    FunctionMismatch::NotFunction => {
                        "this annotation needs a concrete callable witness".to_owned()
                    }
                    FunctionMismatch::Invocation { actual, expected } => {
                        format!(
                            "the {} invocation view cannot supply {}",
                            invocation_label(actual),
                            invocation_label(expected)
                        )
                    }
                    FunctionMismatch::InputCount { actual, expected } => format!(
                        "callable annotation requires {expected} inputs; the selected witness has {actual}"
                    ),
                    FunctionMismatch::Input {
                        position,
                        actual,
                        expected,
                    } => format!(
                        "callable input {} requires {:?} {}; the selected witness supplies {:?} {}",
                        position + 1,
                        expected.operation,
                        self.data_label(expected.data),
                        actual.operation,
                        self.data_label(actual.data)
                    ),
                    FunctionMismatch::Result { actual, expected } => format!(
                        "callable result requires {}; the selected witness returns {}",
                        self.data_label(expected),
                        self.data_label(actual)
                    ),
                    FunctionMismatch::Integrity => return Err(Failure::InconsistentSyntax),
                };
                return self.reject_message(Rule::TypeMismatch, node, message, Vec::new());
            }
        };
        if data == actual.data() {
            return Ok(data);
        }
        if alias {
            let message = "a scoped alias cannot construct a different invocation view; bind the adapted value first";
            return self.reject_message(Rule::TypeMismatch, node, message.to_owned(), Vec::new());
        }
        Ok(data)
    }
}
