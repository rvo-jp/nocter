//! Explicit arguments follow the selected callable's ordered compile-time schema.
use crate::value_access::{Failure, Rule, checker::Checker};
use nocter_declaration_lowering::access_declarations::TypeSyntax;
use nocter_declarations::value_access::{CodeKind, DeclarationId, Namespace};
use nocter_declarations::{GenericParameterDomain, validate_generic_arity};
use nocter_model::{AccessCodeId, BuiltinType, GenericApplication, GenericValue};
use nocter_syntax::{NodeId, NodeKind, SyntaxOrigin, SyntaxToken, direct_node_iter};

impl Checker<'_, '_> {
    pub(super) fn reject_unparameterized_call<T>(&self, arguments: NodeId) -> Result<T, Failure> {
        self.reject_message(
            Rule::InvalidCall,
            arguments,
            "this callable cannot be specialized with explicit generic arguments".into(),
            Vec::new(),
        )
    }

    pub(super) fn explicit_generic_application(
        &mut self,
        code: AccessCodeId,
        list: NodeId,
    ) -> Result<GenericApplication, Failure> {
        let parameters = self
            .program()
            .generic_parameter_ids(code)
            .ok_or(Failure::InconsistentSyntax)?;
        if parameters.is_empty() {
            return self.reject_unparameterized_call(list);
        }
        let arguments = direct_node_iter(self.syntax().tree(), list, NodeKind::GenericArgument)
            .collect::<Vec<_>>();
        if let Err(error) = validate_generic_arity(parameters.len(), arguments.len()) {
            return self.reject_message(Rule::InvalidCall, list, error.to_string(), Vec::new());
        }
        let parameters = parameters.to_vec();
        let mut values = Vec::with_capacity(arguments.len());
        for (&parameter, argument) in parameters.iter().zip(arguments) {
            let domain = self
                .program()
                .generic_parameter(parameter)
                .ok_or(Failure::InconsistentSyntax)?
                .domain();
            let source = self.first_child(argument)?;
            let value = match domain {
                GenericParameterDomain::Type => {
                    if self.kind(source)? != NodeKind::Type {
                        return self.reject_message(
                            Rule::UnknownType,
                            source,
                            "this generic argument requires a type".into(),
                            Vec::new(),
                        );
                    }
                    let Some(data) = self.annotation(source)?.data() else {
                        return self.reject_message(
                            Rule::UnknownType,
                            source,
                            "this generic type argument needs a concrete data identity".into(),
                            Vec::new(),
                        );
                    };
                    GenericValue::Type(data)
                }
                GenericParameterDomain::UsizeConstant => {
                    GenericValue::Usize(self.constant_argument(source)?.into())
                }
            };
            values.push(value);
        }
        Ok(GenericApplication::new(values))
    }

    fn constant_argument(&mut self, source: NodeId) -> Result<u64, Failure> {
        if self.kind(source)? == NodeKind::Expression {
            return self.required_usize(source);
        }
        if let TypeSyntax::Named(path) = self.type_syntax(source)? {
            return self.constant_argument_path(source, &path);
        }
        self.reject_message(
            Rule::TypeMismatch,
            source,
            "this generic argument requires a compile-time usize value".into(),
            Vec::new(),
        )
    }

    fn constant_argument_path(&self, origin: NodeId, path: &[SyntaxToken]) -> Result<u64, Failure> {
        let root = *path.first().ok_or(Failure::InconsistentSyntax)?;
        let spelling = self.text(root)?;
        if self.lookup(spelling).is_some() || self.unavailable.contains(spelling) {
            return self.reject(Rule::RequiredValueScope, root);
        }
        self.require_closed_array_term(root)?;
        if let Some((_, slot)) = self.generic_parameter_at(root)? {
            return match slot.domain() {
                GenericParameterDomain::Type => self.reject(Rule::UnknownName, root),
                GenericParameterDomain::UsizeConstant => self.reject_message(
                    Rule::UnqualifiedGenericCallable,
                    root,
                    "this argument depends on the enclosing generic application".into(),
                    Vec::new(),
                ),
            };
        }
        let declaration = self.declaration_path(Namespace::Value, path)?;
        let DeclarationId::Code(initializer) = declaration else {
            return Err(Failure::InconsistentSyntax);
        };
        self.require_declaration(
            declaration,
            *path.last().ok_or(Failure::InconsistentSyntax)?,
        )?;
        let code = self
            .program()
            .codes()
            .get(initializer)
            .ok_or(Failure::InconsistentSyntax)?;
        if !matches!(
            code.kind(),
            CodeKind::ConstantInitializer | CodeKind::StaticInitializer
        ) || self.program().data_kind(code.result()) != Some(BuiltinType::Usize)
        {
            return self.reject_message(
                Rule::TypeMismatch,
                origin,
                "this generic argument requires a compile-time usize value".into(),
                Vec::new(),
            );
        }
        crate::value_access::required::frozen_usize(
            self.required_initializer_value(initializer, SyntaxOrigin::Node(origin))?,
        )
    }
}
