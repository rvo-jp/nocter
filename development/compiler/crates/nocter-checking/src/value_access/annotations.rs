//! An annotation constrains data or a static callable witness; it never erases the witness.
use super::{Failure, Rule, checker::Checker};
use nocter_declarations::value_access::{
    FunctionConstraint, FunctionInput, FunctionMismatch, Namespace,
};
use nocter_model::{AccessExpressionId, TypeId, ValueInvocation};
use nocter_syntax::{ContextualSpelling, Keyword, NodeId, SyntaxToken};

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
        ty: &nocter_declaration_lowering::access_declarations::DataTypeSyntax,
        metadata_only: bool,
    ) -> Result<TypeId, Failure> {
        use nocter_declaration_lowering::access_declarations::DataTypeSyntax;
        match ty {
            DataTypeSyntax::Named(path) => self.annotation_data_path(path, metadata_only),
            DataTypeSyntax::Optional { payload, layers } => {
                let mut payload = self.annotation_data(payload, metadata_only)?;
                for _ in 0..layers.get() {
                    payload = self
                        .types
                        .optional(payload)
                        .map_err(|_| Failure::InconsistentSyntax)?;
                }
                Ok(payload)
            }
            DataTypeSyntax::Fallible { payload, marker } => {
                let payload = self.annotation_data(payload, metadata_only)?;
                match self.types.fallible(payload) {
                    Ok(data) => Ok(data),
                    Err(nocter_declarations::value_access::BodyTypeError::RepeatedFailure) => self
                        .reject_message(
                        Rule::TypeMismatch,
                        *marker,
                        "one uninterrupted outcome chain cannot contain multiple failure layers"
                            .into(),
                        Vec::new(),
                    ),
                    Err(_) => Err(Failure::InconsistentSyntax),
                }
            }
            DataTypeSyntax::Array { element, length } => {
                let length = self.required_length(*length)?;
                let element = self.annotation_data(element, metadata_only || length == 0)?;
                self.types
                    .array(element, length)
                    .map_err(|_| Failure::InconsistentSyntax)
            }
            DataTypeSyntax::Tuple(fields) => {
                let fields = fields
                    .iter()
                    .map(|field| self.annotation_data(field, metadata_only))
                    .collect::<Result<Vec<_>, _>>()?;
                self.types
                    .tuple(&fields)
                    .map_err(|_| Failure::InconsistentSyntax)
            }
        }
    }
    pub fn annotation(&mut self, node: NodeId) -> Result<Annotation, Failure> {
        use nocter_declaration_lowering::access_declarations::{
            ValueTypeSyntax, value_type_syntax,
        };
        match value_type_syntax(self.syntax(), node).map_err(|_| Failure::InconsistentSyntax)? {
            ValueTypeSyntax::Data(data) => self.annotation_data(&data, false).map(Annotation::Data),
            ValueTypeSyntax::Function(contract) => {
                let inputs = contract
                    .inputs
                    .iter()
                    .map(|(operation, data)| {
                        Ok(FunctionInput {
                            data: self.annotation_data(data, false)?,
                            operation: *operation,
                        })
                    })
                    .collect::<Result<_, Failure>>()?;
                Ok(Annotation::Function(FunctionConstraint {
                    invocation: contract.invocation,
                    inputs,
                    result: self.annotation_data(&contract.result, false)?,
                }))
            }
        }
    }
    pub fn contextual_type(&self, data: TypeId) -> Result<Annotation, Failure> {
        if let Some(nocter_declarations::value_access::PayloadSchema::Function(witness)) =
            self.program().data_schema(data)
        {
            self.program()
                .function_contract(witness)
                .map(Annotation::Function)
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
