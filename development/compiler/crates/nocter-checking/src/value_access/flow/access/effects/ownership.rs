use super::super::super::{
    Operation,
    resources::{self, Holder},
};
use super::{Context, Failure, Result};
use crate::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Referent, SelectionAction,
};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};

impl Context<'_, '_> {
    fn owns_expression(&self, id: AccessExpressionId) -> Result<bool> {
        Ok(self
            .typed
            .body()
            .expressions()
            .get(id)
            .ok_or(Failure::Integrity)?
            .ty
            .normal()
            .is_some_and(|ty| ty.mode() == Mode::Own))
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
            if argument.action == ArgumentAction::AccessOwnedValue {
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
            Operation::IndexFailure(_) => {}
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
            Operation::CommitCall(_) => return Err(Failure::Integrity),
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
            ExpressionKind::Closure { capture, .. } => {
                self.world.resources.capture(
                    self.typed.program(),
                    &Holder::Expression(capture).place(),
                    Holder::Expression(id),
                    resources::expression_payload(self.typed, id)?,
                    &mut self.steps,
                )?;
            }
            ExpressionKind::AdaptFunction { source } => self.world.resources.adapt_function(
                self.typed.program(),
                &Holder::Expression(source).place(),
                Holder::Expression(id),
                resources::expression_payload(self.typed, id)?,
                &mut self.steps,
            )?,
            ExpressionKind::Aggregate { ref fields } => {
                let fields = fields
                    .iter()
                    .map(|field| (field.field, Holder::Expression(field.value).place()))
                    .collect::<Vec<_>>();
                self.world.resources.assemble(
                    self.typed.program(),
                    Holder::Expression(id),
                    resources::expression_payload(self.typed, id)?,
                    &fields,
                    &mut self.steps,
                )?;
            }
            ExpressionKind::Repeat { seed } => self.repeat_ownership(id, seed)?,
            ExpressionKind::Select {
                source,
                ref fields,
                action,
            } => {
                self.select_ownership(id, source, fields, action)?;
            }
            ExpressionKind::Move(place) => self.move_ownership(id, place)?,
            ExpressionKind::RequirePrimary { source }
            | ExpressionKind::Boundary {
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
            ExpressionKind::Index {
                action: crate::value_access::IndexAction::CopyPayload,
                ..
            }
            | ExpressionKind::Function(_)
            | ExpressionKind::Constant(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Unit
            | ExpressionKind::Unary { .. }
            | ExpressionKind::Binary { .. } => self.construct(id)?,
            ExpressionKind::Index {
                action: crate::value_access::IndexAction::Access,
                ..
            }
            | ExpressionKind::Place(_)
            | ExpressionKind::Boundary { .. } => {}
            ExpressionKind::If { .. } | ExpressionKind::Call { .. } => {
                return Err(Failure::Integrity);
            }
        }

        Ok(())
    }
    fn move_ownership(&mut self, id: AccessExpressionId, place: Referent) -> Result<()> {
        if let Referent::Evaluated(selection) = place
            && let Some(frontier) = self.numeric.prefix(self.at, selection)
        {
            let index = resources::RetainedIndex::for_selection(self.typed, selection)
                .ok_or(Failure::Integrity)?;
            self.world.resources.advance_array(
                self.typed,
                index,
                frontier,
                Holder::Expression(id),
                &mut self.steps,
            )?;
        } else {
            self.world.resources.move_to(
                self.typed.program(),
                &self.resource_referent(place)?,
                &Holder::Expression(id).place(),
                false,
                &mut self.steps,
            )?;
        }
        Ok(())
    }
    fn repeat_ownership(&mut self, id: AccessExpressionId, seed: AccessExpressionId) -> Result<()> {
        let payload = resources::expression_payload(self.typed, id)?;
        let Some(nocter_declarations::value_access::PayloadSchema::Array(array)) =
            self.typed.program().data_schema(payload.data())
        else {
            return Err(Failure::Integrity);
        };
        if array.length() > 1 {
            if self.typed.program().data_copyable(array.element()) != Some(true) {
                return Err(Failure::Integrity);
            }
            self.dispose_expression(seed)?;
            return self.construct(id);
        }
        let fields = if array.is_empty() {
            self.dispose_expression(seed)?;
            Vec::new()
        } else {
            vec![(
                nocter_model::ValueField::Array(0),
                Holder::Expression(seed).place(),
            )]
        };
        self.world.resources.assemble(
            self.typed.program(),
            Holder::Expression(id),
            payload,
            &fields,
            &mut self.steps,
        )?;
        Ok(())
    }
    fn select_ownership(
        &mut self,
        id: AccessExpressionId,
        source: AccessExpressionId,
        fields: &nocter_model::FieldPath,
        action: SelectionAction,
    ) -> Result<()> {
        if action == SelectionAction::Access {
            return Ok(());
        }
        let selected = Holder::Expression(source).place().project_path(fields);
        if action == SelectionAction::MovePayload {
            self.world.resources.move_to(
                self.typed.program(),
                &selected,
                &Holder::Expression(id).place(),
                false,
                &mut self.steps,
            )?;
        } else {
            if !self
                .world
                .resources
                .initialized(self.typed.program(), &selected)?
            {
                return Err(Failure::Integrity);
            }
            self.construct(id)?;
        }
        Ok(())
    }
    fn restored_destination(
        &self,
        target: crate::value_access::Referent,
    ) -> Result<Option<resources::Place>> {
        let Some(facts) = self.facts else {
            return Ok(None);
        };
        if let Some(destination) = facts
            .restoration_path(self.typed, target)
            .filter(|destination| self.world.resources.missing_point(destination))
        {
            return Ok(Some(destination));
        }
        let Some(point) = facts.conditional_restoration(self.typed, target) else {
            return Ok(None);
        };
        if let Some(destination) = self.world.resources.unique_array_point(&point.array)? {
            return Ok(Some(destination));
        }
        // Absence of a matching snapshot is the initialized alternative, not permission
        // to read another route's backing or replace an unproved missing element.
        if !self
            .world
            .resources
            .initialized(self.typed.program(), &point.array)?
        {
            return Err(Failure::Integrity);
        }
        Ok(None)
    }
    fn assign_ownership(
        &mut self,
        target: crate::value_access::Referent,
        value: AccessExpressionId,
    ) -> Result<()> {
        let data = self
            .typed
            .body()
            .referent_data(target)
            .ok_or(Failure::Integrity)?;
        let (local, fields) = super::super::super::numeric::path(self.typed, self.facts, target)
            .ok_or(Failure::Integrity)?;
        let owning = self.local_mode(local)? == Mode::Own
            && crate::value_access::paths::concrete(&fields).is_some();
        let restored = self.restored_destination(target)?;
        let destination = if let Some(destination) = restored {
            Some(destination)
        } else if owning {
            Some(self.resource_referent(target)?)
        } else {
            None
        };
        if let Some(destination) = destination {
            self.world.resources.replace(
                self.typed.program(),
                &Holder::Expression(value).place(),
                &destination,
                &mut self.steps,
            )?;
        } else {
            let destination = match target {
                crate::value_access::Referent::Evaluated(id) => Holder::Expression(id).place(),
                crate::value_access::Referent::Place(id) => {
                    let place = self
                        .typed
                        .body()
                        .places()
                        .get(id)
                        .ok_or(Failure::Integrity)?;
                    if self.local_mode(place.local)? != Mode::Update {
                        return Err(Failure::Integrity);
                    }
                    Holder::Local(place.local)
                        .place()
                        .project_path(&place.fields)
                }
            };
            self.world.resources.replace_referent(
                self.typed.program(),
                &Holder::Expression(value).place(),
                &destination,
                data,
                &mut self.steps,
            )?;
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
            ArgumentAction::CopyPlace(_) => {
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
            | ArgumentAction::ReserveTransfer { .. }
            | ArgumentAction::AccessOwnedValue => {}
            ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => {
                return Err(Failure::Integrity);
            }
        }
        Ok(())
    }
    pub(super) fn commit_inputs(&mut self, call: AccessExpressionId) -> Result<()> {
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
        for (index, argument) in arguments.iter().enumerate() {
            let prepared = Holder::Prepared { call, index };
            if let ArgumentAction::ReserveTransfer {
                place,
                ref adapters,
            } = argument.action
            {
                let source = self.materialize_input(place, adapters, prepared)?;
                if argument.operation.access_mode() == Mode::Own {
                    self.world.resources.invoke(
                        self.typed.program(),
                        &source,
                        call,
                        index,
                        &mut self.steps,
                    )?;
                } else {
                    self.world.resources.move_to(
                        self.typed.program(),
                        &source,
                        &Holder::Temporary { call, index }.place(),
                        false,
                        &mut self.steps,
                    )?;
                }
            } else if self.world.resources.contains(prepared) {
                let mode = argument.operation.access_mode();
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
        Ok(())
    }
    /// Replay frozen boundary materializations only after complete bundle validation.
    fn materialize_input(
        &mut self,
        place: Referent,
        adapters: &[AccessExpressionId],
        prepared: Holder,
    ) -> Result<resources::Place> {
        let staging = if let Some(&adapter) = adapters.first() {
            let source = match &self
                .typed
                .body()
                .expressions()
                .get(adapter)
                .ok_or(Failure::Integrity)?
                .kind
            {
                ExpressionKind::AdaptFunction { source } => *source,
                ExpressionKind::Aggregate { fields } => match fields.as_ref() {
                    [
                        crate::value_access::ir::FieldValue {
                            field: nocter_model::ValueField::Variant(true),
                            value,
                        },
                    ] => *value,
                    _ => return Err(Failure::Integrity),
                },
                _ => return Err(Failure::Integrity),
            };
            Holder::Expression(source)
        } else {
            prepared
        };
        let mut source = self.transfer_input_source(place, staging)?;
        for (position, &adapter) in adapters.iter().enumerate() {
            let payload = resources::expression_payload(self.typed, adapter)?;
            // Replay the selected operation, never infer an adapter from type inequality.
            // Complete bundle validation still precedes any source retirement.
            let destination = if position + 1 == adapters.len() {
                prepared
            } else {
                Holder::Expression(adapter)
            };
            match self
                .typed
                .body()
                .expressions()
                .get(adapter)
                .ok_or(Failure::Integrity)?
                .kind
            {
                ExpressionKind::AdaptFunction { .. } => self.world.resources.adapt_function(
                    self.typed.program(),
                    &source,
                    destination,
                    payload,
                    &mut self.steps,
                )?,
                ExpressionKind::Aggregate { .. }
                    if matches!(
                        self.typed.program().data_schema(payload.data()),
                        Some(nocter_declarations::value_access::PayloadSchema::Optional(
                            _
                        ))
                    ) =>
                {
                    self.world.resources.assemble(
                        self.typed.program(),
                        destination,
                        payload,
                        &[(nocter_model::ValueField::Variant(true), source.clone())],
                        &mut self.steps,
                    )?;
                }
                _ => return Err(Failure::Integrity),
            }
            source = destination.place();
        }
        Ok(source)
    }
    fn transfer_input_source(
        &mut self,
        place: Referent,
        destination: Holder,
    ) -> Result<resources::Place> {
        if let Referent::Evaluated(selection) = place
            && let Some(frontier) = self.numeric.prefix(self.at, selection)
        {
            let index = resources::RetainedIndex::for_selection(self.typed, selection)
                .ok_or(Failure::Integrity)?;
            // The bundle was validated before any element retires. Protect this payload in
            // its ordinary argument owner before replacing the array's endpoint, then use
            // the existing invocation/temporary handoff. Cancellation never enters this path.
            self.world.resources.advance_array(
                self.typed,
                index,
                frontier,
                destination,
                &mut self.steps,
            )?;
            Ok(destination.place())
        } else {
            self.resource_referent(place)
        }
    }
    pub(super) fn receive_result(&mut self, call: AccessExpressionId) -> Result<()> {
        if self.owns_expression(call)? {
            let payload = resources::expression_payload(self.typed, call)?;
            self.world
                .resources
                .receive(call, payload, &mut self.steps)?;
        }
        Ok(())
    }
}
