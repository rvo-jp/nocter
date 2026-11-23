//! Owned capture components share one environment; body operations select invocation before flow.
use super::ir::{
    ArgumentAction, Boundary, BoundaryAction, ExpressionKind, FieldValue, Local, LocalOrigin,
    value_type,
};
use super::{Failure, Rule, annotations::Annotation, checker::Checker};
use nocter_declarations::value_access::{FunctionConstraint, FunctionDataPattern, FunctionPattern};
use nocter_model::{
    AccessLocalId, ValueAccessMode, ValueBindingKind, ValueInputOperation, ValueInvocation,
};
use nocter_syntax::{NodeId, NodeKind, SyntaxOrigin};
use std::sync::Arc;
mod inputs;
mod results;

impl Checker<'_, '_> {
    pub fn closure(
        &mut self,
        node: NodeId,
        expected: Option<&FunctionPattern>,
        _reachable: bool,
    ) -> Result<nocter_model::AccessExpressionId, Failure> {
        let head = self.direct(node, NodeKind::ClosureHead)?;
        let (capture_names, captured, data) = self.owned_captures(head)?;
        let result = self.closure_result(node, expected)?;
        let closure = self
            .types
            .reserve_closure()
            .map_err(|_| Failure::InconsistentSyntax)?;
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
        nested.pending_results |= nested.program().payload_readiness(data)
            == nocter_declarations::value_access::PayloadReadiness::Pending;
        let environment = nested.locals.insert(Local {
            data,
            kind: ValueBindingKind::MutableValue,
            origin: LocalOrigin::Capture,
        });
        nested.environment = Some(super::BodyInput {
            local: environment,
            operation: ValueInputOperation::Take,
        });
        nested
            .origins
            .locals
            .insert(environment, nested.site(SyntaxOrigin::Node(head))?);
        nested.bind_captures(&capture_names, environment)?;
        let parameters = self.direct(head, NodeKind::ClosureParameters)?;
        let mut entry = Vec::new();
        let inputs = nested.closure_inputs(parameters, expected, &mut entry)?;
        let boundary = nested
            .result
            .as_ref()
            .and_then(Annotation::data)
            .map(Boundary::Result);
        let hint = nested.result_hint()?;
        let root = nested.block_with_entry(
            self.direct(node, NodeKind::Block)?,
            boundary,
            hint.as_ref(),
            entry,
        )?;
        let result = nested.close_closure_result(node, root)?;
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
    fn bind_captures(
        &mut self,
        names: &[nocter_syntax::SyntaxToken],
        environment: AccessLocalId,
    ) -> Result<(), Failure> {
        let data = self.local_ty(environment).data();
        for (index, token) in names.iter().copied().enumerate() {
            let (fields, component) = if names.len() == 1 {
                (nocter_model::FieldPath::default(), data)
            } else {
                let field = nocter_model::ValueField::Tuple(index);
                let component = self
                    .program()
                    .component(data, field)
                    .ok_or(Failure::InconsistentSyntax)?
                    .data;
                (nocter_model::FieldPath::new([field]), component)
            };
            let place = self.intern_place(environment, fields, component);
            let name = self.text(token)?.to_owned();
            self.scopes[0].insert(name, place);
        }
        Ok(())
    }
    fn owned_captures(
        &mut self,
        head: NodeId,
    ) -> Result<
        (
            Vec<nocter_syntax::SyntaxToken>,
            nocter_model::AccessExpressionId,
            nocter_model::TypeId,
        ),
        Failure,
    > {
        let segment = self.direct(head, NodeKind::ClosureCaptures).ok();
        let names = segment.map(|n| self.children(n)).unwrap_or_default();
        let mut seen = std::collections::BTreeSet::new();
        let mut tokens = Vec::new();
        let mut fields = Vec::new();
        let mut data = Vec::new();
        for (index, capture_node) in names.into_iter().enumerate() {
            let capture_token = self.token(capture_node)?;
            if !seen.insert(self.text(capture_token)?.to_owned()) {
                return self.reject(Rule::ShadowedName, capture_token);
            }
            let place = self.place_name(capture_token, Rule::UnknownName)?;
            let component = self.place_ty(place).data();
            let captured = if self.has(capture_node, nocter_syntax::Punctuation::Caret) {
                self.transfer_place(capture_node, place)?
            } else {
                let captured = self.emit_place(capture_node, place)?;
                self.boundary(capture_node, captured, Boundary::Value(component))?
            };
            super::checker::payload_requirements::data(
                self.program(),
                self.syntax(),
                component,
                SyntaxOrigin::Node(capture_node),
            )?;
            if self.program().payload_dependencies(component).is_none() {
                return self.reject_message(
                    Rule::TypeMismatch,
                    capture_node,
                    "this owned capture needs a valid component descriptor".into(),
                    Vec::new(),
                );
            }
            tokens.push(capture_token);
            fields.push(FieldValue {
                field: nocter_model::ValueField::Tuple(index),
                value: captured,
            });
            data.push(component);
        }
        let captured = match fields.as_slice() {
            [] => self.unit(head)?,
            [single] => single.value,
            _ => {
                let data = self
                    .types
                    .tuple(&data)
                    .map_err(|_| Failure::InconsistentSyntax)?;
                self.emit(
                    head,
                    value_type(data, ValueAccessMode::Own),
                    ExpressionKind::Aggregate {
                        fields: fields.into(),
                    },
                )?
            }
        };
        Ok((
            tokens,
            captured,
            self.ty(captured)
                .access()
                .ok_or(Failure::InconsistentSyntax)?
                .data(),
        ))
    }
    fn closure_result(
        &mut self,
        node: NodeId,
        expected: Option<&FunctionPattern>,
    ) -> Result<Option<Annotation>, Failure> {
        let result = if let Ok(result_node) = self.direct(node, NodeKind::ClosureResult) {
            let result = self.annotation(self.direct(result_node, NodeKind::Type)?)?;
            if let (Some(actual), Some(FunctionDataPattern::Data(expected))) =
                (result.data(), expected.map(|contract| &contract.result))
                && actual != *expected
            {
                return self.reject_message(
                    Rule::TypeMismatch,
                    result_node,
                    "the explicit result data does not match the selected callable contract".into(),
                    Vec::new(),
                );
            }
            Some(result)
        } else {
            if expected.is_some_and(|contract| {
                !matches!(
                    contract.result,
                    FunctionDataPattern::Data(_) | FunctionDataPattern::Function(_)
                )
            }) {
                return self.reject_message(Rule::UnknownType, node, "this compound callable result constraint needs qualified returned-value inference".into(), Vec::new());
            }
            expected.map(|contract| match &contract.result {
                FunctionDataPattern::Data(data) => Annotation::Data(*data),
                FunctionDataPattern::Function(pattern) => Annotation::Function((**pattern).clone()),
                _ => unreachable!("compound result constraints were rejected"),
            })
        };
        Ok(result)
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
    let selects_source = |source: super::Referent| {
        source
            .static_roots(|id| body.expressions().get(id), |id| body.blocks().get(id))
            .is_some_and(|roots| {
                roots.into_iter().any(|root| match root {
                    super::paths::ProjectionRoot::Place(place) => selects(place),
                    super::paths::ProjectionRoot::Temporary(_) => false,
                })
            })
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
                    ExpressionKind::Move(place) if selects_source(*place) => {
                        return Ok(ValueInvocation::Take);
                    }
                    ExpressionKind::Boundary {
                        source,
                        action: BoundaryAction::DelegateUpdate,
                    } if body
                        .expressions()
                        .get(*source)
                        .and_then(|e| e.referent(*source))
                        .is_some_and(selects_source) =>
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
                        ArgumentAction::ReserveTransfer { place, .. } if selects_source(place) => {
                            return Ok(ValueInvocation::Take);
                        }
                        ArgumentAction::ReserveUpdate(place) if selects_source(place) => {
                            invocation = ValueInvocation::Edit;
                        }
                        _ => {}
                    }
                }
                Operation::Assign { target, .. } | Operation::CompoundAssign { target, .. }
                    if selects_source(target) =>
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
