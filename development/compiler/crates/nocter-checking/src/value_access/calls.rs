//! Select concrete or formal dispatch once; all inputs share the same preparation contract.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Argument, ArgumentAction, CallArguments, CallKind, ExpressionKind, value_type},
};
use nocter_declaration_lowering::access_declarations::{TypeMember, TypeMemberKind};
use nocter_declarations::value_access::{
    FunctionConstraint, FunctionInput, FunctionWitness, Namespace, PayloadSchema,
};
use nocter_model::{
    AccessCodeId, AccessExpressionId, TypeId, ValueAccessMode as Mode, ValueFunctionOrigin,
    ValueInvocation,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxOrigin, SyntaxToken, TokenKind};

mod comparison;
mod conversion;
mod generics;
mod receiver;

struct Callee {
    witness: FunctionWitness,
    source: CalleeSource,
}

#[derive(Clone, Copy)]
enum CalleeSource {
    Declared {
        token: Option<SyntaxToken>,
        receiver: Option<(NodeId, AccessExpressionId)>,
    },
    Value {
        environment: AccessExpressionId,
        input: FunctionInput,
    },
}

impl Callee {
    fn direct(
        target: AccessCodeId,
        token: SyntaxToken,
        receiver: Option<(NodeId, AccessExpressionId)>,
    ) -> Self {
        Self {
            witness: FunctionWitness {
                origin: ValueFunctionOrigin::Declaration(target),
                invocation: ValueInvocation::Read,
            },
            source: CalleeSource::Declared {
                token: Some(token),
                receiver,
            },
        }
    }
}
impl Checker<'_, '_> {
    fn declared_function(
        &self,
        node: NodeId,
    ) -> Result<Option<(AccessCodeId, SyntaxToken)>, Failure> {
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
            .flatten()
            .map(|code| (code, name)))
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
            self.require_nongeneric_callable(target, token)?;
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
                .required_initializer_value(target, nocter_syntax::SyntaxOrigin::Node(node))?
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
            (_, nocter_model::FrozenNode::Enum { tag, fields })
                if matches!(
                    self.program().data_schema(data),
                    Some(PayloadSchema::Enumeration(schema))
                        if usize::try_from(*tag).ok()
                            .and_then(|tag| schema.variants().get(tag))
                            .and_then(|&variant| self.program().variant_components(data, variant))
                            .is_some_and(|components| components.len() == fields.len())
                ) =>
            {
                ExpressionKind::Constant(value.clone())
            }
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
        let callee = self.unwrap(node)?;
        if self.kind(callee)? == NodeKind::PostfixExpression
            && let [base, suffix] = self.children(callee).as_slice()
            && self.kind(*suffix)? == NodeKind::MemberSuffix
        {
            let token = self.member_token(*suffix)?;
            if token.kind() == TokenKind::Identifier
                && let Some(owner) = self.constructor_owner(*base)?
            {
                let Some(member) = self.context.type_member(owner, self.text(token)?) else {
                    return self.reject(Rule::InvalidCall, token);
                };
                return self.type_member_callee(member, token, None);
            }
            if let Some(mut selected) = self.qualified_selection(callee)? {
                if selected.members.pop().is_some() {
                    let receiver = self.qualified_value(*base, &selected)?;
                    return self.member_callee(callee, *base, receiver, token);
                }
                if self
                    .program()
                    .codes()
                    .get(selected.code)
                    .is_some_and(|code| code.kind().is_callable())
                {
                    return Ok(Callee::direct(selected.code, selected.token, None));
                }
                let value = self.qualified_value(callee, &selected)?;
                return self.value_callee(callee, value);
            }
            return self.prepare_member_callee(callee, *base, token, reachable);
        }
        if let Some((target, token)) = self.declared_function(node)? {
            return Ok(Callee::direct(target, token, None));
        }
        let value = self.expression(node, None, reachable)?;
        self.value_callee(node, value)
    }
    fn type_member_callee(
        &self,
        member: TypeMember,
        token: SyntaxToken,
        receiver: Option<(NodeId, AccessExpressionId)>,
    ) -> Result<Callee, Failure> {
        self.require_type_member(member, token, receiver.is_some())?;
        Ok(Callee::direct(member.code, token, receiver))
    }
    fn require_type_member(
        &self,
        member: TypeMember,
        token: SyntaxToken,
        receiver: bool,
    ) -> Result<(), Failure> {
        if !member.accessible {
            return self.reject(Rule::InaccessibleName, token);
        }
        let mismatch = match (member.kind, receiver) {
            (TypeMemberKind::Method, false) => {
                Some("a method must be called on a value of its owning type")
            }
            (TypeMemberKind::Construction, true) => {
                Some("a construction function must be called on its owning type")
            }
            _ => None,
        };
        if let Some(message) = mismatch {
            return self.reject_message(Rule::InvalidCall, token, message.into(), Vec::new());
        }
        self.require_declaration(member.code.into(), token)
    }
    fn member_for(&self, data: TypeId, token: SyntaxToken) -> Result<Option<TypeMember>, Failure> {
        let Some(owner) = self.program().behavior_owner(data) else {
            return Ok(None);
        };
        Ok(self.context.type_member(owner, self.text(token)?))
    }
    fn member_callee(
        &mut self,
        node: NodeId,
        base: NodeId,
        receiver: AccessExpressionId,
        token: SyntaxToken,
    ) -> Result<Callee, Failure> {
        if let Some(access) = self.ty(receiver).access()
            && let Some(member) = self.member_for(access.data(), token)?
        {
            return self.type_member_callee(member, token, Some((base, receiver)));
        }
        let value = self.component_value(node, receiver, &[token])?;
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
            source: CalleeSource::Value {
                environment: value,
                input: FunctionInput {
                    data: access.data(),
                    operation: witness.invocation.operation(),
                },
            },
        })
    }
    pub fn call(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [callee, suffix] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        if let Some((member, variant)) = self.qualified_enum_variant(*callee)? {
            if let Ok(list) = self.direct(*suffix, NodeKind::GenericArguments) {
                return self.reject_unparameterized_call(list);
            }
            let arguments = self.children(*suffix);
            return self.enum_value(node, member, variant, Some(&arguments), reachable);
        }
        let selected = self.callee(*callee, reachable)?;
        self.call_with_suffix(node, *suffix, &selected, reachable)
    }
    pub(super) fn call_value(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        value: AccessExpressionId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let selected = self.value_callee(node, value)?;
        self.call_with_suffix(node, suffix, &selected, reachable)
    }
    pub(super) fn call_member_value(
        &mut self,
        call: NodeId,
        arguments: NodeId,
        selection: NodeId,
        receiver: AccessExpressionId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let children = self.children(selection);
        let [base, member] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let token = self.member_token(*member)?;
        let selected = self.member_callee(selection, *base, receiver, token)?;
        self.call_with_suffix(call, arguments, &selected, reachable)
    }
    fn local_future_argument(&self, input: FunctionInput, plan: &Argument) -> bool {
        use nocter_model::ValueInputOperation;
        let supported = matches!(
            (input.operation, &plan.action),
            (ValueInputOperation::Take, _)
                | (
                    ValueInputOperation::Read | ValueInputOperation::Edit,
                    ArgumentAction::AccessOwnedValue
                )
                | (ValueInputOperation::Read, ArgumentAction::ReserveRead(_))
                | (ValueInputOperation::Edit, ArgumentAction::ReserveUpdate(_))
        );
        supported
            && super::flow::resources::local_future_input(
                self.program(),
                input.data,
                input.operation,
            )
            .is_some()
    }

    fn call_with_suffix(
        &mut self,
        node: NodeId,
        suffix: NodeId,
        selected: &Callee,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let generic = self.direct(suffix, NodeKind::GenericArguments).ok();
        let mut arguments = self.children(suffix);
        arguments.retain(|&argument| Some(argument) != generic);
        self.selected_call(node, &arguments, selected, generic, reachable)
    }

    fn selected_call(
        &mut self,
        node: NodeId,
        arguments: &[NodeId],
        selected: &Callee,
        generic: Option<NodeId>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let (contract, application) = if let Some(arguments) = generic {
            let (CalleeSource::Declared { .. }, ValueFunctionOrigin::Declaration(code)) =
                (selected.source, selected.witness.origin)
            else {
                return self.reject_unparameterized_call(arguments);
            };
            let application = self.explicit_generic_application(code, arguments)?;
            let Ok(contract) = self.types.apply_callable_signature(code, &application) else {
                return self.reject_message(
                    Rule::TypeMismatch,
                    arguments,
                    "these generic arguments do not produce a valid concrete callable signature"
                        .into(),
                    Vec::new(),
                );
            };
            let header = self
                .program()
                .codes()
                .get(code)
                .ok_or(Failure::InconsistentSyntax)?;
            if !matches!(
                header.kind(),
                nocter_declarations::value_access::CodeKind::Function(
                    nocter_model::CompileTimeGuarantee::RuntimeOnly
                )
            ) {
                return self.reject(Rule::UnqualifiedGenericCallable, arguments);
            }
            (contract, Some(application))
        } else {
            if let ValueFunctionOrigin::Declaration(code) = selected.witness.origin {
                self.require_nongeneric_callable(code, node)?;
            }
            let contract = self
                .program()
                .function_contract(selected.witness)
                .ok_or(Failure::InconsistentSyntax)?;
            (contract, None)
        };
        let (kind, mut arguments, continues) =
            self.prepare_call_arguments(node, arguments, selected.source, &contract, reachable)?;
        arguments.application = application;
        self.emit_selected_call(node, selected, &contract, kind, arguments, continues)
    }

    fn prepare_call_arguments(
        &mut self,
        node: NodeId,
        arguments: &[NodeId],
        source: CalleeSource,
        contract: &FunctionConstraint,
        reachable: bool,
    ) -> Result<(CallKind, CallArguments, bool), Failure> {
        let (receiver, inputs) = match source {
            CalleeSource::Declared {
                receiver: Some(receiver),
                ..
            } => {
                let (input, inputs) = contract
                    .inputs
                    .split_first()
                    .ok_or(Failure::InconsistentSyntax)?;
                (Some((receiver, *input)), inputs)
            }
            _ => (None, contract.inputs.as_ref()),
        };
        if arguments.len() != inputs.len() {
            return self.reject(Rule::InvalidCall, node);
        }
        let mut plans = Vec::new();
        let (kind, mut continues) = if let CalleeSource::Value { environment, input } = source {
            plans.push(self.input_plan(self.first_child(node)?, environment, input, None)?);
            (CallKind::Value, self.ty(environment).may_complete())
        } else if let Some(((syntax, value), input)) = receiver {
            plans.push(self.prepare_input(syntax, value, input)?);
            (CallKind::Direct, self.ty(value).may_complete())
        } else {
            (CallKind::Direct, true)
        };
        for (syntax, expected) in arguments.iter().copied().zip(inputs.iter().copied()) {
            let value = self.input_expression(syntax, expected, reachable && continues)?;
            continues &= self.ty(value).may_complete();
            plans.push(self.prepare_input(syntax, value, expected)?);
        }
        Ok((kind, CallArguments::in_order(plans), continues))
    }

    fn require_nongeneric_callable(
        &self,
        code: AccessCodeId,
        origin: impl super::checker::Authored,
    ) -> Result<(), Failure> {
        if self
            .program()
            .generic_parameter_ids(code)
            .ok_or(Failure::InconsistentSyntax)?
            .is_empty()
        {
            Ok(())
        } else {
            self.reject(Rule::UnqualifiedGenericCallable, origin)
        }
    }

    fn emit_selected_call(
        &mut self,
        node: NodeId,
        selected: &Callee,
        contract: &FunctionConstraint,
        kind: CallKind,
        arguments: CallArguments,
        continues: bool,
    ) -> Result<AccessExpressionId, Failure> {
        if super::instances::completed(&arguments) {
            let callee = self.first_child(node)?;
            if matches!(
                self.program().function_execution(selected.witness.origin),
                Some(nocter_declarations::CallableExecution::Deferred { .. })
            ) {
                if kind != CallKind::Direct
                    || !contract
                        .inputs
                        .iter()
                        .zip(&arguments)
                        .all(|(input, plan)| self.local_future_argument(*input, plan))
                {
                    return self.reject(Rule::UnqualifiedFuture, callee);
                }
            } else {
                super::checker::payload_requirements::invocation(
                    self.program(),
                    self.syntax(),
                    contract,
                    nocter_syntax::SyntaxOrigin::Node(callee),
                )?;
            }
        }
        let requires_application = matches!(
            self.program().data_schema(contract.result),
            Some(PayloadSchema::Function(_))
        ) || self
            .program()
            .payload_dependencies(contract.result)
            .is_some_and(|dependencies| {
                dependencies
                    .producers()
                    .iter()
                    .any(|witness| matches!(witness.origin, ValueFunctionOrigin::Result(_)))
            });
        // A type contract constrains the returned value; it cannot own a result producer.
        // Original call-result sources retain the selected body's actual return context.
        let result = if requires_application
            && !matches!(selected.witness.origin, ValueFunctionOrigin::Contract(_))
        {
            self.types
                .application(self.code, self.expressions.next_id(), selected.witness)
                .map_err(|_| Failure::InconsistentSyntax)?
        } else {
            contract.result
        };
        let call = self.emit(
            node,
            value_type(result, Mode::Own).with_continuation(continues),
            ExpressionKind::Call {
                kind,
                target: selected.witness.origin,
                arguments,
            },
        )?;
        if let CalleeSource::Declared {
            token: Some(token), ..
        } = selected.source
        {
            let site = self.site(SyntaxOrigin::Token(token))?;
            self.origins.direct_calls.insert(call, site);
        }
        Ok(call)
    }
}
