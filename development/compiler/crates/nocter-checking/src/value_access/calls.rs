//! Select concrete or formal dispatch once; all inputs share the same preparation contract.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{CallKind, ExpressionKind, value_type},
};
use nocter_declarations::value_access::{FunctionInput, FunctionWitness, Namespace, PayloadSchema};
use nocter_model::{
    AccessCodeId, AccessExpressionId, ValueAccessMode as Mode, ValueFunctionOrigin, ValueInvocation,
};
use nocter_syntax::NodeId;

struct Callee {
    witness: FunctionWitness,
    environment: Option<(AccessExpressionId, FunctionInput)>,
    completes: bool,
}
impl Checker<'_, '_> {
    fn declared_function(&self, node: NodeId) -> Result<Option<AccessCodeId>, Failure> {
        let Some(name) = self.name_token(node)? else {
            return Ok(None);
        };
        self.require_header(Namespace::Value, name)?;
        let spelling = self.text(name)?;
        let scope = self.scope_at(name)?;
        Ok(self
            .lookup(spelling)
            .is_none()
            .then(|| {
                self.program()
                    .symbols()
                    .get(spelling)
                    .and_then(|name| self.program().resolve_in(scope, name))
            })
            .flatten())
    }
    pub fn reference_value(&mut self, node: NodeId) -> Result<AccessExpressionId, Failure> {
        let token = self.name_token(node)?.ok_or(Failure::InconsistentSyntax)?;
        self.require_header(Namespace::Value, token)?;
        let scope = self.scope_at(token)?;
        if self.lookup(self.text(token)?).is_none()
            && let Some(name) = self.program().symbols().get(self.text(token)?)
        {
            if self.program().has_namespace_in(scope, name) {
                return self.reject(Rule::NamespaceValue, token);
            }
            if let Some(code) = self.program().resolve_code_in(scope, name) {
                return self.code_value(node, code, token);
            }
        }
        self.place_value(node, Rule::TypeMismatch)
    }
    pub(super) fn code_value(
        &mut self,
        node: NodeId,
        target: AccessCodeId,
        token: nocter_syntax::SyntaxToken,
    ) -> Result<AccessExpressionId, Failure> {
        self.require_declaration(target.into(), token)?;
        let code = self
            .program()
            .codes()
            .get(target)
            .ok_or(Failure::InconsistentSyntax)?;
        if code.kind().is_callable() {
            let data = self
                .program()
                .function_data(FunctionWitness {
                    origin: ValueFunctionOrigin::Declaration(target),
                    invocation: ValueInvocation::Read,
                })
                .ok_or(Failure::InconsistentSyntax)?;
            return self.emit(
                node,
                value_type(data, Mode::Own),
                ExpressionKind::Function(target),
            );
        }
        if code.kind() == nocter_declarations::value_access::CodeKind::ConstantInitializer {
            let data = code.result();
            let value = self
                .values
                .value(target.into())
                .ok_or(Failure::RequiredConstant {
                    initializer: target,
                    origin: nocter_syntax::SyntaxOrigin::Node(node),
                })?
                .clone();
            return self.constant_value(node, data, &value);
        }
        if code.kind() == nocter_declarations::value_access::CodeKind::StaticInitializer {
            let place = self.static_place(
                nocter_model::ValueStatic {
                    initializer: target,
                },
                token,
            )?;
            return self.emit_place(node, place);
        }
        Err(Failure::InconsistentSyntax)
    }
    pub(super) fn qualified_value(
        &mut self,
        node: NodeId,
        selected: &super::qualified::QualifiedSelection,
    ) -> Result<AccessExpressionId, Failure> {
        if self
            .program()
            .codes()
            .get(selected.code)
            .ok_or(Failure::InconsistentSyntax)?
            .kind()
            == nocter_declarations::value_access::CodeKind::StaticInitializer
        {
            let place = self.static_place(
                nocter_model::ValueStatic {
                    initializer: selected.code,
                },
                selected.token,
            )?;
            let place = self.member_place(place, &selected.members)?;
            return self.emit_place(node, place);
        }
        let source = self.code_value(selected.node, selected.code, selected.token)?;
        if selected.members.is_empty() {
            Ok(source)
        } else {
            self.component_value(node, source, &selected.members)
        }
    }
    pub(super) fn constant_value(
        &mut self,
        node: NodeId,
        data: nocter_model::TypeId,
        value: &nocter_model::FrozenData,
    ) -> Result<AccessExpressionId, Failure> {
        let kind = match (self.program().data_kind(data), value.node()) {
            (
                Some(builtin),
                nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Integer(value)),
            ) if self.integer_data(data) => {
                let (min, max) = nocter_model::BuiltinNumericType::for_builtin(builtin)
                    .and_then(nocter_model::BuiltinNumericType::integer_bounds)
                    .ok_or(Failure::InconsistentSyntax)?;
                if *value < min || *value > max {
                    return Err(Failure::InconsistentSyntax);
                }
                ExpressionKind::Integer(*value)
            }
            (
                Some(nocter_model::BuiltinType::Bool),
                nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Bool(value)),
            ) => ExpressionKind::Boolean(*value),
            (Some(nocter_model::BuiltinType::Void), nocter_model::FrozenNode::Unit) => {
                ExpressionKind::Unit
            }
            (
                Some(nocter_model::BuiltinType::Str),
                nocter_model::FrozenNode::Scalar(nocter_model::ConstantValue::Text(_)),
            ) => ExpressionKind::Constant(value.clone()),
            (_, nocter_model::FrozenNode::Variant { .. })
                if matches!(
                    self.program().data_schema(data),
                    Some(PayloadSchema::Optional(_) | PayloadSchema::Fallible(_))
                ) =>
            {
                ExpressionKind::Constant(value.clone())
            }
            (_, nocter_model::FrozenNode::Aggregate(_))
                if matches!(
                    self.program().data_schema(data),
                    Some(PayloadSchema::Aggregate(_) | PayloadSchema::Array(_))
                ) =>
            {
                ExpressionKind::Constant(value.clone())
            }
            (_, nocter_model::FrozenNode::Repeated { .. })
                if matches!(
                    self.program().data_schema(data),
                    Some(PayloadSchema::Array(_))
                ) =>
            {
                ExpressionKind::Constant(value.clone())
            }
            _ => return Err(Failure::InconsistentSyntax),
        };
        self.emit(node, value_type(data, Mode::Own), kind)
    }
    fn callee(&mut self, node: NodeId, reachable: bool) -> Result<Callee, Failure> {
        if let Some(selected) = self.qualified_selection(node)? {
            if selected.members.is_empty()
                && self
                    .program()
                    .codes()
                    .get(selected.code)
                    .is_some_and(|code| code.kind().is_callable())
            {
                return Ok(Callee {
                    witness: FunctionWitness {
                        origin: ValueFunctionOrigin::Declaration(selected.code),
                        invocation: ValueInvocation::Read,
                    },
                    environment: None,
                    completes: true,
                });
            }
            let value = self.qualified_value(node, &selected)?;
            return self.value_callee(node, value);
        }
        if let Some(target) = self.declared_function(node)? {
            return Ok(Callee {
                witness: FunctionWitness {
                    origin: ValueFunctionOrigin::Declaration(target),
                    invocation: ValueInvocation::Read,
                },
                environment: None,
                completes: true,
            });
        }
        let value = self.expression(node, None, reachable)?;
        self.value_callee(node, value)
    }
    fn value_callee(&self, node: NodeId, value: AccessExpressionId) -> Result<Callee, Failure> {
        let Some(access) = self.ty(value).access() else {
            return self.reject(Rule::InvalidCall, node);
        };
        let Some(PayloadSchema::Function(witness)) = self.program().data_schema(access.data())
        else {
            return self.reject(Rule::InvalidCall, node);
        };
        Ok(Callee {
            witness,
            completes: self.ty(value).may_complete(),
            environment: Some((
                value,
                FunctionInput {
                    data: access.data(),
                    operation: witness.invocation.operation(),
                },
            )),
        })
    }
    pub fn call(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [callee, suffix] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let selected = self.callee(*callee, reachable)?;
        self.selected_call(node, *suffix, &selected, reachable)
    }
    pub(super) fn call_value(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        value: AccessExpressionId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let selected = self.value_callee(node, value)?;
        self.selected_call(node, suffix, &selected, reachable)
    }
    fn selected_call(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        selected: &Callee,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let contract = self
            .program()
            .function_contract(selected.witness)
            .ok_or(Failure::InconsistentSyntax)?;
        let arguments = self.children(suffix);
        if arguments.len() != contract.inputs.len() {
            return self.reject(Rule::InvalidCall, node);
        }
        let mut plans = Vec::new();
        let mut continues = selected.completes;
        let kind = if selected.environment.is_some() {
            CallKind::Value
        } else {
            CallKind::Direct
        };
        if let Some((value, expected)) = selected.environment {
            plans.push(self.input_plan(self.first_child(node)?, value, expected, None)?);
        }
        for (syntax, expected) in arguments.into_iter().zip(contract.inputs) {
            let value = self.input_expression(syntax, expected, reachable && continues)?;
            continues &= self.ty(value).may_complete();
            plans.push(self.prepare_input(syntax, value, expected)?);
        }
        let result = if matches!(
            self.program().data_schema(contract.result),
            Some(PayloadSchema::Function(_))
        ) {
            self.types
                .application(self.code, self.expressions.next_id(), selected.witness)
                .map_err(|_| Failure::InconsistentSyntax)?
        } else {
            contract.result
        };
        self.emit(
            node,
            value_type(result, Mode::Own).with_continuation(continues),
            ExpressionKind::Call {
                kind,
                target: selected.witness.origin,
                arguments: plans.into_boxed_slice(),
            },
        )
    }
}
