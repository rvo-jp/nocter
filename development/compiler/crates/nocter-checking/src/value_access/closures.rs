//! One explicit owned capture: body operations select invocation before flow validation.
use super::ir::{
    ArgumentAction, Boundary, BoundaryAction, ExpressionKind, Local, LocalOrigin, value_type,
};
use super::{Failure, Rule, annotations::Annotation, checker::Checker};
use nocter_declarations::value_access::{FunctionConstraint, FunctionInput, PayloadSchema};
use nocter_model::{
    AccessLocalId, ValueAccessMode, ValueBindingKind, ValueClosure, ValueInputOperation,
    ValueInvocation,
};
use nocter_syntax::{ContextualSpelling, NodeId, NodeKind, SyntaxOrigin};
use std::sync::Arc;

impl Checker<'_, '_> {
    pub fn closure(
        &mut self,
        node: NodeId,
        _reachable: bool,
    ) -> Result<nocter_model::AccessExpressionId, Failure> {
        if matches!(self.code, nocter_model::ValueCode::Closure(_)) {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "nested closure producers require a later environment scope".into(),
                Vec::new(),
            );
        }
        let head = self.direct(node, NodeKind::ClosureHead)?;
        let (capture_token, captured, data) = self.owned_capture(head)?;
        let capture_name = self.text(capture_token)?.to_owned();
        let result_node = self.direct(node, NodeKind::ClosureResult).ok();
        let Some(result_node) = result_node else {
            return self.reject_message(
                Rule::UnknownType,
                node,
                "this private closure scope needs an explicit result data type".into(),
                Vec::new(),
            );
        };
        let Annotation::Data(result) =
            self.annotation(self.direct(result_node, NodeKind::Type)?)?
        else {
            return self.reject(Rule::UnknownType, result_node);
        };
        let closure = ValueClosure {
            owner: self.types.owner(),
            slot: self
                .closures
                .len()
                .try_into()
                .map_err(|_| Failure::InconsistentSyntax)?,
        };
        let mut nested = Checker::new(
            self.context,
            result,
            self.types.clone(),
            self.projection.clone(),
            self.source.clone(),
            nocter_model::ValueCode::Closure(closure),
            self.values,
        );
        nested.unavailable.clone_from(&self.unavailable);
        let environment = nested.locals.insert(Local {
            data,
            kind: ValueBindingKind::MutableValue,
            origin: LocalOrigin::Capture,
        });
        nested.environment = Some(super::BodyInput {
            local: environment,
            operation: ValueInputOperation::Take,
        });
        nested.origins.locals.insert(
            environment,
            nested.site(SyntaxOrigin::Token(capture_token))?,
        );
        nested.scopes[0].insert(capture_name, environment);
        let parameters = self.direct(head, NodeKind::ClosureParameters)?;
        let inputs = nested.closure_inputs(parameters)?;
        let root = nested.block(
            self.direct(node, NodeKind::Block)?,
            Some(Boundary::Result(result)),
            true,
        )?;
        self.types = nested.types.clone();
        let mut control = super::flow::Control::prepare(Arc::new(nested.finish(root)))
            .map_err(|_| Failure::InconsistentSyntax)?;
        let invocation = invocation(&control, environment)?;
        let contract = FunctionConstraint {
            invocation,
            inputs: inputs.into(),
            result,
        };
        let closure_data = self
            .types
            .closure(closure, data, contract)
            .map_err(|_| Failure::InconsistentSyntax)?;
        let typed = Arc::make_mut(&mut control.typed);
        typed.body.inputs.environment = Some(super::BodyInput {
            local: environment,
            operation: invocation.operation(),
        });
        typed.body.locals = typed.body.locals.map(|id, local| {
            let mut local = local.clone();
            if id == environment {
                local.kind = invocation.operation().binding_kind();
            }
            local
        });
        typed.program = Arc::new(self.program().clone());
        self.closures.insert(closure, control);
        self.emit(
            node,
            value_type(closure_data, ValueAccessMode::Own)
                .with_continuation(self.ty(captured).may_complete()),
            ExpressionKind::Closure {
                closure,
                capture: captured,
            },
        )
    }
    fn owned_capture(
        &mut self,
        head: NodeId,
    ) -> Result<
        (
            nocter_syntax::SyntaxToken,
            nocter_model::AccessExpressionId,
            nocter_model::TypeId,
        ),
        Failure,
    > {
        let segment = self.direct(head, NodeKind::ClosureCaptures).ok();
        let names = segment.map(|n| self.children(n)).unwrap_or_default();
        let [capture_node] = names.as_slice() else {
            return self.reject_message(
                Rule::TypeMismatch,
                head,
                "this private closure scope requires exactly one explicit value capture".into(),
                Vec::new(),
            );
        };
        let capture_token = self.token(*capture_node)?;
        let local = self.place_name(capture_token, Rule::UnknownName)?;
        let place = self.intern_place(
            local,
            nocter_model::FieldPath::default(),
            self.local_ty(local).data(),
        );
        let data = self.place_ty(place).data();
        if matches!(
            self.program().data_schema(data),
            Some(PayloadSchema::Function(_))
        ) {
            return self.reject_message(
                Rule::TypeMismatch,
                *capture_node,
                "this owned-capture scope admits ordinary closed data, not callable environments"
                    .into(),
                Vec::new(),
            );
        }
        let captured = if self.has(*capture_node, nocter_syntax::Punctuation::Caret) {
            self.transfer_place(*capture_node, place)?
        } else {
            let captured = self.emit_place(*capture_node, place)?;
            self.boundary(*capture_node, captured, Boundary::Value(data))?
        };
        Ok((capture_token, captured, data))
    }
    fn closure_inputs(&mut self, parameters: NodeId) -> Result<Vec<FunctionInput>, Failure> {
        let mut inputs = Vec::new();
        for parameter in self.children(parameters) {
            let token = self.token(parameter)?;
            let name = self.text(token)?.to_owned();
            if self.shadows_name(&name) {
                return self.reject(Rule::ShadowedName, token);
            }
            let Some(ty) = self.direct(parameter, NodeKind::Type).ok() else {
                return self.reject_message(
                    Rule::UnknownType,
                    parameter,
                    "this private closure scope needs explicit parameter data types".into(),
                    Vec::new(),
                );
            };
            let Annotation::Data(data) = self.annotation(ty)? else {
                return self.reject(Rule::UnknownType, ty);
            };
            let operation = if let Ok(operation) = self.direct(parameter, NodeKind::InputOperation)
            {
                match ContextualSpelling::from_spelling(self.text(self.token(operation)?)?) {
                    Some(ContextualSpelling::Edit) => ValueInputOperation::Edit,
                    Some(ContextualSpelling::Take) => ValueInputOperation::Take,
                    _ => return Err(Failure::InconsistentSyntax),
                }
            } else {
                ValueInputOperation::Read
            };
            let local = self.locals.insert(Local {
                data,
                kind: operation.binding_kind(),
                origin: LocalOrigin::ClosureParameter,
            });
            self.inputs.push(super::BodyInput { local, operation });
            self.origins
                .locals
                .insert(local, self.site(SyntaxOrigin::Token(token))?);
            self.scopes[0].insert(name, local);
            inputs.push(FunctionInput { data, operation });
        }
        Ok(inputs)
    }
}

fn invocation(
    control: &super::flow::Control,
    environment: AccessLocalId,
) -> Result<ValueInvocation, Failure> {
    use super::flow::Operation;
    let body = control.typed.body();
    let selects = |place| {
        body.places()
            .get(place)
            .is_some_and(|p| p.local == environment)
    };
    let mut invocation = ValueInvocation::Read;
    let mut pending = std::collections::BTreeSet::from([control.graph.entry()]);
    let mut visited = std::collections::BTreeSet::new();
    while let Some(point) = pending.pop_first() {
        if !visited.insert(point) {
            continue;
        }
        let block = control
            .graph
            .points()
            .get(point)
            .ok_or(Failure::InconsistentSyntax)?;
        for action in &block.actions {
            match action.operation {
                Operation::Evaluate(id) => match &body
                    .expressions()
                    .get(id)
                    .ok_or(Failure::InconsistentSyntax)?
                    .kind
                {
                    ExpressionKind::Move(place) if selects(*place) => {
                        return Ok(ValueInvocation::Take);
                    }
                    ExpressionKind::Boundary {
                        source,
                        action: BoundaryAction::DelegateUpdate,
                    } if body
                        .expressions()
                        .get(*source)
                        .and_then(|e| e.referent(*source))
                        .and_then(|source| source.static_base(|id| body.expressions().get(id)))
                        .is_some_and(selects) =>
                    {
                        invocation = ValueInvocation::Edit;
                    }
                    _ => {}
                },
                Operation::PrepareArgument { call, argument } => {
                    let ExpressionKind::Call { arguments, .. } = &body
                        .expressions()
                        .get(call)
                        .ok_or(Failure::InconsistentSyntax)?
                        .kind
                    else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    match arguments
                        .get(argument)
                        .ok_or(Failure::InconsistentSyntax)?
                        .action
                    {
                        ArgumentAction::ReserveTransfer { place, .. }
                            if place
                                .static_base(|id| body.expressions().get(id))
                                .is_some_and(selects) =>
                        {
                            return Ok(ValueInvocation::Take);
                        }
                        ArgumentAction::ReserveUpdate(place)
                            if place
                                .static_base(|id| body.expressions().get(id))
                                .is_some_and(selects) =>
                        {
                            invocation = ValueInvocation::Edit;
                        }
                        _ => {}
                    }
                }
                Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. }
                    if selects(target) =>
                {
                    invocation = ValueInvocation::Edit;
                }
                Operation::Drop(local) if local == environment => return Ok(ValueInvocation::Take),
                _ => {}
            }
        }
        pending.extend(block.terminator.successors());
    }
    Ok(invocation)
}
