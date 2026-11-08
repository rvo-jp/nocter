//! Select static dispatch once and prepare invocation environments through the ordinary input plan.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Argument, ArgumentAction, ExpressionKind, value_type},
};
use nocter_declarations::value_access::{FunctionInput, FunctionWitness, PayloadSchema};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode, ValueInvocation};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    pub fn reference_value(&mut self, node: NodeId) -> Result<AccessExpressionId, Failure> {
        let name = self.name_token(node)?.ok_or(Failure::InconsistentSyntax)?;
        let spelling = self.text(name)?;
        if self.lookup(spelling).is_none()
            && let Some(target) = self
                .program()
                .symbols()
                .get(spelling)
                .and_then(|name| self.program().resolve(name))
        {
            let data = self
                .program()
                .function_data(FunctionWitness {
                    declaration: target,
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

    pub fn call(&mut self, node: NodeId, reachable: bool) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let [callee, suffix] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let direct = if let Some(name) = self.name_token(*callee)? {
            let spelling = self.text(name)?;
            self.lookup(spelling)
                .is_none()
                .then(|| {
                    self.program()
                        .symbols()
                        .get(spelling)
                        .and_then(|name| self.program().resolve(name))
                })
                .flatten()
        } else {
            None
        };
        let mut plans = Vec::new();
        let mut continues = true;
        let mut environment = None;
        let target = if let Some(target) = direct {
            target
        } else {
            let value = self.expression(*callee, None, reachable)?;
            let Some(access) = self.ty(value).access() else {
                return self.reject(Rule::InvalidCall, *callee);
            };
            let Some(PayloadSchema::Function(witness)) = self.program().data_schema(access.data())
            else {
                return self.reject(Rule::InvalidCall, *callee);
            };
            continues = self.ty(value).may_complete();
            environment = Some((
                value,
                FunctionInput {
                    data: access.data(),
                    operation: witness.invocation.operation(),
                },
            ));
            witness.declaration
        };
        let declaration = self
            .program()
            .callables()
            .get(target)
            .ok_or(Failure::InconsistentSyntax)?;
        let expected = declaration
            .parameters()
            .iter()
            .map(|id| {
                let p = self
                    .program()
                    .parameters()
                    .get(*id)
                    .expect("frozen parameter");
                FunctionInput {
                    data: p.data(),
                    operation: p.operation(),
                }
            })
            .collect::<Vec<_>>();
        let result = declaration.result();
        let arguments = self.children(*suffix);
        if arguments.len() != expected.len() {
            return self.reject(Rule::InvalidCall, node);
        }
        if let Some((value, expected)) = environment {
            plans.push(self.input_plan(*callee, value, expected)?);
        }
        for (syntax, expected) in arguments.into_iter().zip(expected) {
            let value = self.expression(syntax, None, reachable && continues)?;
            continues &= self.ty(value).may_complete();
            plans.push(self.input_plan(syntax, value, expected)?);
        }
        self.emit(
            node,
            value_type(result, Mode::Own).with_continuation(continues),
            ExpressionKind::Call {
                target,
                arguments: plans.into_boxed_slice(),
            },
        )
    }

    fn input_plan(
        &self,
        syntax: NodeId,
        value: AccessExpressionId,
        expected: FunctionInput,
    ) -> Result<Argument, Failure> {
        self.require_data(value, expected.data, syntax)?;
        let mode = expected.operation.access_mode();
        let action = if let Some(actual) = self.ty(value).normal() {
            if let Some(place) = self.place_expression(value) {
                match mode {
                    Mode::Read => ArgumentAction::ReserveRead(place),
                    Mode::Update => {
                        if !self.writable_place(place) {
                            return self.reject_local(
                                Rule::InvalidUpdate,
                                syntax,
                                self.selected_place(place).local,
                            );
                        }
                        ArgumentAction::ReserveUpdate(place)
                    }
                    Mode::Own if self.copyable(actual.data()) => ArgumentAction::CopyPayload,
                    Mode::Own if self.place_ty(place).mode() == Mode::Own => {
                        ArgumentAction::ReserveTransfer(place)
                    }
                    Mode::Own => return self.reject(Rule::MissingTransfer, syntax),
                }
            } else {
                if actual.mode() != Mode::Own {
                    return Err(Failure::InconsistentSyntax);
                }
                match mode {
                    Mode::Read => ArgumentAction::ReadOwnedValue,
                    Mode::Own => ArgumentAction::TakeValue,
                    Mode::Update => return self.reject(Rule::InvalidUpdate, syntax),
                }
            }
        } else {
            ArgumentAction::Interrupted
        };
        Ok(Argument {
            value,
            operation: expected.operation,
            action,
        })
    }
}
