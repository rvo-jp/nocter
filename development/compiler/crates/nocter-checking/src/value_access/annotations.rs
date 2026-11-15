//! An annotation constrains data or a static callable witness; it never erases the witness.
use super::{Failure, Rule, checker::Checker};
use nocter_declarations::value_access::{
    FunctionConstraint, FunctionMismatch, FunctionPattern, FunctionPatternMismatch, Namespace,
};
use nocter_model::{AccessExpressionId, TypeId, ValueInvocation};
use nocter_syntax::{ContextualSpelling, Keyword, NodeId, SyntaxToken};
mod patterns;

pub(super) fn invocation_label(invocation: ValueInvocation) -> &'static str {
    match invocation {
        ValueInvocation::Read => Keyword::Func.as_str(),
        ValueInvocation::Edit => ContextualSpelling::Mut.as_str(),
        ValueInvocation::Take => ContextualSpelling::Once.as_str(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Annotation {
    Data(TypeId),
    Function(FunctionPattern),
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
    fn annotation_data_token(
        &self,
        token: SyntaxToken,
        metadata_only: bool,
    ) -> Result<TypeId, Failure> {
        if !metadata_only {
            self.require_header(Namespace::Type, token)?;
        }
        let scope = self.scope_at(token)?;
        let data = if metadata_only {
            self.program()
                .type_identity_named_in(scope, self.text(token)?)
        } else {
            self.program().type_named_in(scope, self.text(token)?)
        };
        data.map_or_else(|| self.reject(Rule::UnknownType, token), Ok)
    }
    pub(super) fn annotation_data_path(
        &self,
        path: &[SyntaxToken],
        metadata_only: bool,
    ) -> Result<TypeId, Failure> {
        if let [token] = path {
            return self.annotation_data_token(*token, metadata_only);
        }
        let declaration = self.declaration_path(Namespace::Type, path)?;
        let nocter_declarations::value_access::DeclarationId::Record(record) = declaration else {
            return Err(Failure::InconsistentSyntax);
        };
        if !metadata_only {
            self.require_declaration(
                declaration,
                *path.last().ok_or(Failure::InconsistentSyntax)?,
            )?;
        }
        self.program()
            .record_type_identity(record)
            .ok_or(Failure::InconsistentSyntax)
    }
    fn annotation_data(
        &mut self,
        ty: &nocter_declaration_lowering::access_declarations::TypeSyntax,
        metadata_only: bool,
    ) -> Result<TypeId, Failure> {
        self.annotation_data_in(ty, metadata_only, &mut None)
    }
    fn annotation_data_in(
        &mut self,
        ty: &nocter_declaration_lowering::access_declarations::TypeSyntax,
        metadata_only: bool,
        input: &mut Option<(usize, usize)>,
    ) -> Result<TypeId, Failure> {
        use nocter_declaration_lowering::access_declarations::TypeSyntax;
        match ty {
            TypeSyntax::Callable(contract) => {
                let Some((position, ordinal)) = input else {
                    return self.reject(Rule::UnsupportedData, contract.head);
                };
                let pattern = self.callable_annotation_in(contract, metadata_only)?;
                let data = self.closure_input_data(
                    Annotation::Function(pattern),
                    *position,
                    Some(*ordinal),
                    nocter_syntax::SyntaxOrigin::Token(contract.head),
                )?;
                *ordinal += 1;
                Ok(data)
            }
            TypeSyntax::Named(path) => self.annotation_data_path(path, metadata_only),
            TypeSyntax::Optional { payload, layers } => {
                let mut payload = self.annotation_data_in(payload, metadata_only, input)?;
                for _ in 0..layers.get() {
                    payload = self
                        .types
                        .optional(payload)
                        .map_err(|_| Failure::InconsistentSyntax)?;
                }
                Ok(payload)
            }
            TypeSyntax::Fallible { payload, marker } => {
                let payload = self.annotation_data_in(payload, metadata_only, input)?;
                match self.types.fallible(payload) {
                    Ok(data) => Ok(data),
                    Err(nocter_declarations::value_access::BodyTypeError::RepeatedFailure) => {
                        self.reject_repeated_failure(*marker)
                    }
                    Err(_) => Err(Failure::InconsistentSyntax),
                }
            }
            TypeSyntax::Array { element, length } => {
                let length = self.required_length(*length)?;
                let element =
                    self.annotation_data_in(element, metadata_only || length == 0, input)?;
                self.types
                    .array(element, length)
                    .map_err(|_| Failure::InconsistentSyntax)
            }
            TypeSyntax::Tuple(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| self.annotation_data_in(field, metadata_only, input))
                    .collect::<Result<Vec<_>, _>>()?;
                self.types
                    .tuple(&fields)
                    .map_err(|_| Failure::InconsistentSyntax)
            }
        }
    }
    fn reject_repeated_failure<T>(&self, marker: SyntaxToken) -> Result<T, Failure> {
        self.reject_message(
            Rule::TypeMismatch,
            marker,
            "one uninterrupted outcome chain cannot contain multiple failure layers".into(),
            Vec::new(),
        )
    }
    fn annotation_syntax(
        &self,
        node: NodeId,
    ) -> Result<nocter_declaration_lowering::access_declarations::TypeSyntax, Failure> {
        use nocter_declaration_lowering::access_declarations::{TypeSyntax, project_type_syntax};
        let ty =
            project_type_syntax(self.syntax(), node).map_err(|_| Failure::InconsistentSyntax)?;
        let mut pending = vec![(&ty, false)];
        while let Some((ty, failure)) = pending.pop() {
            match ty {
                TypeSyntax::Fallible { payload, marker } => {
                    if failure {
                        return self.reject_repeated_failure(*marker);
                    }
                    pending.push((payload, true));
                }
                TypeSyntax::Optional { payload, .. } => pending.push((payload, failure)),
                TypeSyntax::Tuple(fields) => {
                    pending.extend(fields.iter().rev().map(|field| (field, false)));
                }
                TypeSyntax::Array { element, .. } => pending.push((element, false)),
                TypeSyntax::Callable(contract) => {
                    pending.push((&contract.result, false));
                    pending.extend(contract.inputs.iter().rev().map(|(_, data)| (data, false)));
                }
                TypeSyntax::Named(_) => {}
            }
        }
        Ok(ty)
    }
    pub fn annotation(&mut self, node: NodeId) -> Result<Annotation, Failure> {
        use nocter_declaration_lowering::access_declarations::TypeSyntax;
        match self.annotation_syntax(node)? {
            TypeSyntax::Callable(contract) => self
                .callable_annotation(&contract)
                .map(Annotation::Function),
            data => self.annotation_data(&data, false).map(Annotation::Data),
        }
    }
    pub(super) fn closure_input_annotation(
        &mut self,
        node: NodeId,
        position: usize,
    ) -> Result<Annotation, Failure> {
        use nocter_declaration_lowering::access_declarations::TypeSyntax;
        match self.annotation_syntax(node)? {
            TypeSyntax::Callable(contract) => self
                .callable_annotation(&contract)
                .map(Annotation::Function),
            data => self
                .annotation_data_in(&data, false, &mut Some((position, 0)))
                .map(Annotation::Data),
        }
    }
    pub fn contextual_type(&self, data: TypeId) -> Result<Annotation, Failure> {
        if let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
            self.program().data_schema(data)
        {
            self.program()
                .function_contract(witness)
                .map(|contract| Annotation::Function(FunctionPattern::exact(contract)))
                .ok_or(Failure::InconsistentSyntax)
        } else {
            Ok(Annotation::Data(data))
        }
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
        let result = self
            .program()
            .constrain_function(actual.data(), expected)
            .map_err(|mismatch| FunctionPatternMismatch {
                path: Box::new([]),
                mismatch,
            });
        self.finish_function_constraint(node, actual.data(), result, alias)
    }
    pub(super) fn function_pattern_data(
        &self,
        node: NodeId,
        value: AccessExpressionId,
        expected: &FunctionPattern,
        alias: bool,
    ) -> Result<TypeId, Failure> {
        let Some(actual) = self.ty(value).access() else {
            return self.reject(Rule::TypeMismatch, node);
        };
        self.finish_function_constraint(
            node,
            actual.data(),
            self.program()
                .constrain_function_pattern(actual.data(), expected),
            alias,
        )
    }
    fn finish_function_constraint(
        &self,
        node: NodeId,
        actual: TypeId,
        result: Result<TypeId, FunctionPatternMismatch>,
        alias: bool,
    ) -> Result<TypeId, Failure> {
        let data = match result {
            Ok(data) => data,
            Err(failure) => {
                let mut message = String::new();
                for position in failure.path {
                    use nocter_declarations::value_access::FunctionPosition;
                    use std::fmt::Write;
                    match position {
                        FunctionPosition::Input(index) => {
                            write!(message, "callable input {} -> ", index + 1)
                        }
                        FunctionPosition::Result => write!(message, "callable result -> "),
                        FunctionPosition::Component(component) => match component {
                            nocter_model::ValueProjection::Tuple(index) => {
                                write!(message, "tuple field {} -> ", index + 1)
                            }
                            nocter_model::ValueProjection::Array(()) => {
                                write!(message, "array element -> ")
                            }
                            nocter_model::ValueProjection::Variant(true) => {
                                write!(message, "primary payload -> ")
                            }
                            _ => return Err(Failure::InconsistentSyntax),
                        },
                    }
                    .expect("writing into a string");
                }
                message.push_str(&self.function_mismatch_message(failure.mismatch)?);
                return self.reject_message(Rule::TypeMismatch, node, message, Vec::new());
            }
        };
        if data == actual {
            return Ok(data);
        }
        if alias {
            let message = "a scoped alias cannot construct a different invocation view; bind the adapted value first";
            return self.reject_message(Rule::TypeMismatch, node, message.to_owned(), Vec::new());
        }
        Ok(data)
    }
    fn function_mismatch_message(&self, mismatch: FunctionMismatch) -> Result<String, Failure> {
        Ok(match mismatch {
            FunctionMismatch::Data { actual, expected } => format!(
                "component requires {}; the selected witness supplies {}",
                self.data_label(expected),
                self.data_label(actual)
            ),
            FunctionMismatch::Structure { actual, expected } => {
                use nocter_declarations::value_access::FunctionStructure;
                let expected = match expected {
                    FunctionStructure::Optional => "an optional value".to_owned(),
                    FunctionStructure::Fallible => "a fallible value".to_owned(),
                    FunctionStructure::Tuple(length) => format!("a tuple with {length} fields"),
                    FunctionStructure::Array(length) => format!("an array with {length} elements"),
                };
                format!(
                    "component requires {expected}; the selected witness supplies {}",
                    self.data_label(actual)
                )
            }
            FunctionMismatch::NotFunction => {
                "this annotation needs a selected callable witness".to_owned()
            }
            FunctionMismatch::Invocation { actual, expected } => {
                format!(
                    "the {} invocation view cannot supply {}",
                    invocation_label(actual),
                    invocation_label(expected)
                )
            }
            FunctionMismatch::ContractInvocation { actual, expected } => format!(
                "nested callable contract requires {} invocation; the selected contract declares {}",
                invocation_label(expected),
                invocation_label(actual)
            ),
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
        })
    }
}
