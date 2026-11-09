//! Select concrete or formal dispatch once; all inputs share the same preparation contract.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{CallKind, ExpressionKind, value_type},
};
use nocter_declarations::value_access::{FunctionInput, FunctionWitness, PayloadSchema};
use nocter_model::{
    AccessCallableId, AccessExpressionId, ValueAccessMode as Mode, ValueFunctionOrigin,
    ValueInvocation,
};
use nocter_syntax::NodeId;

struct Callee {
    witness: FunctionWitness,
    environment: Option<(AccessExpressionId, FunctionInput)>,
    completes: bool,
}
impl Checker<'_, '_> {
    fn declared_function(&self, node: NodeId) -> Result<Option<AccessCallableId>, Failure> {
        let Some(name) = self.name_token(node)? else {
            return Ok(None);
        };
        let spelling = self.text(name)?;
        Ok(self
            .lookup(spelling)
            .is_none()
            .then(|| {
                self.program()
                    .symbols()
                    .get(spelling)
                    .and_then(|name| self.program().resolve(name))
            })
            .flatten())
    }
    pub fn reference_value(&mut self, node: NodeId) -> Result<AccessExpressionId, Failure> {
        if let Some(target) = self.declared_function(node)? {
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
        self.place_value(node, Rule::TypeMismatch)
    }
    fn callee(&mut self, node: NodeId, reachable: bool) -> Result<Callee, Failure> {
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
        let contract = self
            .program()
            .function_contract(selected.witness)
            .ok_or(Failure::InconsistentSyntax)?;
        if matches!(self.code, nocter_model::ValueCode::Closure(_))
            && super::results::is_result(self.program(), contract.result)
        {
            return self.reject_message(
                Rule::TypeMismatch,
                node,
                "callable results inside closure bodies require a later witness scope".to_owned(),
                Vec::new(),
            );
        }
        let arguments = self.children(*suffix);
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
            plans.push(self.input_plan(*callee, value, expected, None)?);
        }
        for (syntax, expected) in arguments.into_iter().zip(contract.inputs) {
            let value = self.expression(syntax, None, reachable && continues)?;
            continues &= self.ty(value).may_complete();
            plans.push(self.prepare_input(syntax, value, expected)?);
        }
        let result = match selected.witness.origin {
            ValueFunctionOrigin::Declaration(callee)
                if self
                    .program()
                    .callables()
                    .get(callee)
                    .is_some_and(|callable| callable.result_contract().is_some()) =>
            {
                self.types
                    .application(self.expressions.next_id(), callee)
                    .map_err(|_| Failure::InconsistentSyntax)?
            }
            _ => contract.result,
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
