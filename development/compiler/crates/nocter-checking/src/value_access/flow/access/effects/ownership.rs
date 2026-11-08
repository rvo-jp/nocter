use super::super::super::{
    Operation,
    resources::{self, Holder, Step},
};
use super::super::Root;
use super::{Context, Failure, Result};
use crate::value_access::{ArgumentAction, BoundaryAction, ExpressionKind, ValueType};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};

impl Context<'_, '_> {
    fn owns_expression(&self, id: AccessExpressionId) -> Result<bool> {
        Ok(
            matches!(self.typed.body().expressions().get(id).ok_or(Failure::Integrity)?.ty,
            ValueType::Value(ty) if ty.mode() == Mode::Own),
        )
    }
    fn construct(&mut self, id: AccessExpressionId) -> Result<()> {
        if self.owns_expression(id)? {
            let payload = resources::expression_payload(self.typed, id)?;
            self.world
                .resources
                .construct(Holder::Expression(id), payload, &mut self.steps)?;
        }
        Ok(())
    }
    fn dispose_expression(&mut self, id: AccessExpressionId) -> Result<()> {
        if self.owns_expression(id)? {
            self.world.resources.dispose(
                self.typed.program(),
                &Holder::Expression(id).place(),
                &mut self.steps,
            )?;
        }
        Ok(())
    }
    pub(super) fn ownership_before(&mut self, operation: &Operation) -> Result<()> {
        if let Operation::PrepareArgument {
            call,
            argument: index,
        } = *operation
        {
            let ExpressionKind::Call { arguments, .. } = &self
                .typed
                .body()
                .expressions()
                .get(call)
                .ok_or(Failure::Integrity)?
                .kind
            else {
                return Err(Failure::Integrity);
            };
            let argument = arguments.get(index).ok_or(Failure::Integrity)?;
            if argument.action == ArgumentAction::ReadOwnedValue {
                self.world.resources.move_to(
                    self.typed.program(),
                    &Holder::Expression(argument.value).place(),
                    &Holder::Prepared { call, index }.place(),
                    false,
                    &mut self.steps,
                )?;
            }
        }
        Ok(())
    }
    pub(super) fn ownership_after(&mut self, operation: &Operation) -> Result<()> {
        match *operation {
            Operation::BeginEvaluation(id) => self.world.resources.begin(id)?,
            Operation::EndEvaluation(id) => {
                for holder in self.world.resources.ending(id)? {
                    self.world.resources.dispose(
                        self.typed.program(),
                        &holder.place(),
                        &mut self.steps,
                    )?;
                }
                self.world.resources.end(id)?;
            }
            Operation::Evaluate(id) => self.evaluate_ownership(id)?,
            Operation::PrepareOperand { value, .. }
            | Operation::Condition(value)
            | Operation::CompoundAssign { value, .. }
            | Operation::Discard(value) => self.dispose_expression(value)?,
            Operation::BooleanConstant { result, .. } => self.construct(result)?,
            Operation::BooleanAnswer { source, result } => {
                // The branch predicate was already read into a scalar snapshot.
                if self.world.resources.contains(Holder::Expression(source)) {
                    self.dispose_expression(source)?;
                }
                self.construct(result)?;
            }
            Operation::Forward { source, result } => {
                if self.owns_expression(source)? {
                    self.world.resources.move_to(
                        self.typed.program(),
                        &Holder::Expression(source).place(),
                        &Holder::Expression(result).place(),
                        true,
                        &mut self.steps,
                    )?;
                }
            }
            Operation::Bind { local, value } => {
                if self.local_mode(local)? == Mode::Own {
                    self.world.resources.move_to(
                        self.typed.program(),
                        &Holder::Expression(value).place(),
                        &Holder::Local(local).place(),
                        false,
                        &mut self.steps,
                    )?;
                }
            }
            Operation::Assign { target, value } => self.assign_ownership(target, value)?,
            Operation::Drop(local) => self.world.resources.dispose(
                self.typed.program(),
                &Holder::Local(local).place(),
                &mut self.steps,
            )?,
            Operation::StageReturn(value) => self.world.resources.move_to(
                self.typed.program(),
                &Holder::Expression(value).place(),
                &Holder::Return.place(),
                false,
                &mut self.steps,
            )?,
            Operation::ExitScope { ref locals, .. } => {
                for &local in locals {
                    if self.world.resources.contains(Holder::Local(local)) {
                        self.world.resources.dispose(
                            self.typed.program(),
                            &Holder::Local(local).place(),
                            &mut self.steps,
                        )?;
                    }
                }
            }
            Operation::PrepareArgument {
                call,
                argument: index,
            } => self.prepare_ownership(call, index)?,
            Operation::CommitCall(call) => self.commit_ownership(call)?,
            Operation::CancelCall { call, prepared } => {
                for index in (0..prepared).rev() {
                    let holder = Holder::Prepared { call, index };
                    if self.world.resources.contains(holder) {
                        self.world.resources.dispose(
                            self.typed.program(),
                            &holder.place(),
                            &mut self.steps,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    fn evaluate_ownership(&mut self, id: AccessExpressionId) -> Result<()> {
        let kind = &self
            .typed
            .body()
            .expressions()
            .get(id)
            .ok_or(Failure::Integrity)?
            .kind;
        match *kind {
            ExpressionKind::Move(local) => self.world.resources.move_to(
                self.typed.program(),
                &Holder::Local(local).place(),
                &Holder::Expression(id).place(),
                false,
                &mut self.steps,
            )?,
            ExpressionKind::Boundary {
                source,
                action: BoundaryAction::TakeValue,
            } => self.world.resources.move_to(
                self.typed.program(),
                &Holder::Expression(source).place(),
                &Holder::Expression(id).place(),
                false,
                &mut self.steps,
            )?,
            ExpressionKind::Boundary {
                source,
                action: BoundaryAction::CopyPayload,
            } => {
                self.dispose_expression(source)?;
                self.construct(id)?;
            }
            ExpressionKind::Place(_) | ExpressionKind::Boundary { .. } => {}
            ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Unit
            | ExpressionKind::Unary { .. }
            | ExpressionKind::Binary { .. } => self.construct(id)?,
            ExpressionKind::If { .. } | ExpressionKind::Call { .. } => {
                return Err(Failure::Integrity);
            }
        }

        Ok(())
    }
    fn assign_ownership(
        &mut self,
        target: nocter_model::AccessLocalId,
        value: AccessExpressionId,
    ) -> Result<()> {
        let (root, _) = self.base(target)?.ok_or(Failure::Integrity)?;
        match root {
            Root::Owned(local) => {
                self.world.resources.replace(
                    self.typed.program(),
                    &Holder::Expression(value).place(),
                    &Holder::Local(local).place(),
                    &mut self.steps,
                )?;
            }
            Root::Input(parameter) => {
                let source = Holder::Expression(value).place();
                let payload = self.world.resources.take(self.typed.program(), &source)?;
                self.steps.push(Step::ReplaceInput {
                    parameter,
                    source,
                    payload,
                });
            }
            Root::Temporary { .. } => return Err(Failure::Integrity),
        }

        Ok(())
    }
    fn prepare_ownership(&mut self, call: AccessExpressionId, index: usize) -> Result<()> {
        let ExpressionKind::Call { arguments, .. } = &self
            .typed
            .body()
            .expressions()
            .get(call)
            .ok_or(Failure::Integrity)?
            .kind
        else {
            return Err(Failure::Integrity);
        };
        let argument = arguments.get(index).ok_or(Failure::Integrity)?;
        let holder = Holder::Prepared { call, index };
        match argument.action {
            ArgumentAction::CopyPayload => {
                self.dispose_expression(argument.value)?;
                let payload = resources::expression_payload(self.typed, argument.value)?;
                self.world
                    .resources
                    .construct(holder, payload, &mut self.steps)?;
            }
            ArgumentAction::TakeValue => self.world.resources.move_to(
                self.typed.program(),
                &Holder::Expression(argument.value).place(),
                &holder.place(),
                false,
                &mut self.steps,
            )?,
            ArgumentAction::ReserveRead(_)
            | ArgumentAction::ReserveUpdate(_)
            | ArgumentAction::ReserveTransfer(_)
            | ArgumentAction::ReadOwnedValue => {}
            ArgumentAction::Interrupted => return Err(Failure::Integrity),
        }
        Ok(())
    }
    fn commit_ownership(&mut self, call: AccessExpressionId) -> Result<()> {
        let ExpressionKind::Call { target, arguments } = &self
            .typed
            .body()
            .expressions()
            .get(call)
            .ok_or(Failure::Integrity)?
            .kind
        else {
            return Err(Failure::Integrity);
        };
        let declaration = self
            .typed
            .program()
            .callables()
            .get(*target)
            .ok_or(Failure::Integrity)?;
        for (index, argument) in arguments.iter().enumerate() {
            let prepared = Holder::Prepared { call, index };
            if let ArgumentAction::ReserveTransfer(local) = argument.action {
                self.world.resources.invoke(
                    self.typed.program(),
                    &Holder::Local(local).place(),
                    call,
                    index,
                    &mut self.steps,
                )?;
            } else if self.world.resources.contains(prepared) {
                let mode = self
                    .typed
                    .program()
                    .parameters()
                    .get(
                        *declaration
                            .parameters()
                            .get(index)
                            .ok_or(Failure::Integrity)?,
                    )
                    .ok_or(Failure::Integrity)?
                    .operation()
                    .access_mode();
                if mode == Mode::Own {
                    self.world.resources.invoke(
                        self.typed.program(),
                        &prepared.place(),
                        call,
                        index,
                        &mut self.steps,
                    )?;
                } else {
                    self.world.resources.move_to(
                        self.typed.program(),
                        &prepared.place(),
                        &Holder::Temporary { call, index }.place(),
                        false,
                        &mut self.steps,
                    )?;
                }
            }
        }
        if self.owns_expression(call)? {
            let payload = resources::expression_payload(self.typed, call)?;
            self.world
                .resources
                .receive(call, payload, &mut self.steps)?;
        }
        Ok(())
    }
}
