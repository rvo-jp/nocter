use super::super::super::{
    Operation, cancelled_slots,
    resources::{self, ActivationAccess, ActivationAccessSource, ActivationInput, Holder},
};
use super::super::{
    AccessRule, Violation,
    liveness::Carrier,
    state::{Prepared, PreparedSource},
};
use super::{Context, Failure, Result};
use crate::value_access::{
    Argument, ArgumentAction, BoundaryAction, ExpressionKind, Referent, SelectionAction,
};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode, ValueInputOperation};

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
    fn end_evaluation_ownership(&mut self, id: AccessExpressionId) -> Result<()> {
        for holder in self.world.resources.ending(id)? {
            let root = match holder {
                Holder::Temporary { call, index } => Some(super::Root::Temporary { call, index }),
                Holder::Expression(source) => Some(super::Root::Expression(source)),
                _ => None,
            };
            if let Some(root) = root {
                if let Some(access) = self
                    .world
                    .resources
                    .retained_accesses()
                    .find(|access| access.region.root == root)
                {
                    return Err(Violation {
                        location: self.at,
                        rule: AccessRule::ExpiredRoot,
                        root,
                        fields: access.region.fields.clone(),
                        retained: Some(access.retained_access()),
                    }
                    .into());
                }
                self.write(&root.region(), None, Mode::Own, None)?;
            }
            self.world
                .resources
                .dispose(self.typed.program(), &holder.place(), &mut self.steps)?;
        }
        self.world.resources.end(id)?;
        Ok(())
    }
    pub(super) fn ownership_before(&mut self, operation: &Operation) -> Result<()> {
        if let Operation::DriveFuture { future, result } = *operation {
            let operand = self.world.value(future)?;
            if operand.mode != Mode::Own || operand.access.is_some() {
                return Err(Failure::Integrity);
            }
            self.write(
                &super::Root::Expression(future).region(),
                None,
                Mode::Own,
                None,
            )?;
            let output = self
                .typed
                .body()
                .expressions()
                .get(result)
                .and_then(|expression| expression.ty.normal())
                .filter(|output| output.mode() == Mode::Own)
                .ok_or(Failure::Integrity)?;
            self.world.resources.drive_future(
                self.typed.program(),
                future,
                output.data(),
                &mut self.steps,
            )?;
        }
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
            Operation::EndEvaluation(id) => self.end_evaluation_ownership(id)?,
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
            Operation::Forward {
                source,
                result,
                scope_exit,
            } => {
                if self.owns_expression(source)? {
                    self.world.resources.move_to(
                        self.typed.program(),
                        &Holder::Expression(source).place(),
                        &Holder::Expression(result).place(),
                        scope_exit,
                        &mut self.steps,
                    )?;
                } else if scope_exit && let Some(key) = self.world.value(result)?.access {
                    let region = &self.world.nodes.get(&key).ok_or(Failure::Integrity)?.region;
                    if let super::super::Root::Expression(owner) = region.root {
                        self.world.resources.protect_expression(owner, source)?;
                    }
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
                        self.destroy(local)?;
                        self.world.resources.dispose(
                            self.typed.program(),
                            &Holder::Local(local).place(),
                            &mut self.steps,
                        )?;
                    }
                    self.world.values.remove(&Carrier::Local(local));
                }
            }
            Operation::PrepareArgument {
                call,
                argument: index,
            } => self.prepare_ownership(call, index)?,
            Operation::CommitCall(_) => return Err(Failure::Integrity),
            Operation::ConstructFuture(call) => self.construct_activation(call)?,
            Operation::DriveFuture { result, .. } => self.world.resources.receive(
                result,
                resources::expression_payload(self.typed, result)?,
                &mut self.steps,
            )?,
            Operation::CancelCall { call, prepared } => {
                for index in cancelled_slots(self.typed, call, prepared)?.rev() {
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
            ExpressionKind::Aggregate { ref fields } => self.aggregate_ownership(id, fields)?,
            ExpressionKind::Repeat { seed } => self.repeat_ownership(id, seed)?,
            ExpressionKind::Select {
                source,
                ref fields,
                action,
            } => {
                self.select_ownership(id, source, fields, action)?;
            }
            ExpressionKind::Move(place) => self.move_ownership(id, place)?,
            ExpressionKind::Boundary {
                source,
                action: BoundaryAction::TakeValue,
            } if self.typed.result_evidence.as_ref().is_some_and(|evidence| {
                evidence.reclassifies_expression(self.typed, source, id)
            }) =>
            {
                self.transport_ownership(id, source)?;
            }
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
            | ExpressionKind::PrimaryCondition { .. }
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Unit
            | ExpressionKind::Unary { .. }
            | ExpressionKind::Binary { .. } => self.construct(id)?,
            ExpressionKind::Index {
                action: crate::value_access::IndexAction::Access,
                ..
            }
            | ExpressionKind::GuardView { .. }
            | ExpressionKind::Place(_)
            | ExpressionKind::Boundary { .. } => {}
            ExpressionKind::Scoped { .. }
            | ExpressionKind::Block { .. }
            | ExpressionKind::If { .. }
            | ExpressionKind::Call { .. }
            | ExpressionKind::Await { .. } => {
                return Err(Failure::Integrity);
            }
        }

        Ok(())
    }
    fn transport_ownership(
        &mut self,
        id: AccessExpressionId,
        source: AccessExpressionId,
    ) -> Result<()> {
        let typed = self.typed;
        self.world.resources.transport(
            typed.program(),
            &Holder::Expression(source).place(),
            Holder::Expression(id),
            resources::expression_payload(typed, id)?,
            &mut self.steps,
            |from, to| {
                typed
                    .result_evidence
                    .as_ref()
                    .is_some_and(|evidence| evidence.permits_transport(typed.program(), from, to))
            },
        )?;
        Ok(())
    }
    fn aggregate_ownership(
        &mut self,
        id: AccessExpressionId,
        fields: &[crate::value_access::ir::FieldValue],
    ) -> Result<()> {
        let fields = fields
            .iter()
            .map(|field| (field.field, Holder::Expression(field.value).place()))
            .collect::<Vec<_>>();
        let typed = self.typed;
        self.world.resources.assemble_with(
            self.typed.program(),
            Holder::Expression(id),
            resources::expression_payload(self.typed, id)?,
            &fields,
            &mut self.steps,
            |actual, expected| {
                typed
                    .result_evidence
                    .as_ref()
                    .map_or(actual == expected, |evidence| {
                        evidence.matches_payload_metadata(typed.program(), actual, expected)
                    })
            },
        )?;
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
        if let Some(destination) = self
            .world
            .resources
            .unique_array_point(&point.array, &point.suffix)?
        {
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
        let owning = if let Some((local, fields)) =
            super::super::super::numeric::path(self.typed, self.facts, target)
        {
            self.local_mode(local)? == Mode::Own
                && crate::value_access::paths::concrete(&fields).is_some()
        } else {
            false
        };
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
            | ArgumentAction::ReuseRead { .. }
            | ArgumentAction::AccessOwnedValue => {}
            ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => {
                return Err(Failure::Integrity);
            }
        }
        Ok(())
    }
    fn construct_activation(&mut self, call: AccessExpressionId) -> Result<()> {
        let preparations = self.commit_preparations(call)?;
        let activation = resources::future_activation(self.typed, call)?;
        if preparations.len() != activation.inputs.len() {
            return Err(Failure::Integrity);
        }
        let mut sources = Vec::with_capacity(activation.inputs.len());
        for (index, &(argument, expected)) in activation.inputs.iter().enumerate() {
            if argument.operation != ValueInputOperation::Take {
                sources.push(ActivationInput::Access(self.retain_activation_access(
                    call,
                    index,
                    argument,
                    expected.data(),
                    &preparations[index],
                )?));
                continue;
            }
            let prepared = Holder::Prepared { call, index }.place();
            let source = self
                .committed_input_source(call, index, argument)?
                .ok_or(Failure::Integrity)?;
            if source != prepared {
                self.world.resources.move_to(
                    self.typed.program(),
                    &source,
                    &prepared,
                    false,
                    &mut self.steps,
                )?;
            }
            sources.push(ActivationInput::Take {
                source: prepared,
                payload: expected,
            });
        }
        self.world.resources.construct_activation(
            self.typed.program(),
            Holder::Expression(call),
            activation.payload,
            &sources,
            &mut self.steps,
        )?;
        Ok(())
    }
    fn retain_activation_access(
        &mut self,
        call: AccessExpressionId,
        index: usize,
        argument: &Argument,
        data: nocter_model::TypeId,
        prepared: &Prepared,
    ) -> Result<ActivationAccess> {
        let PreparedSource::Reserved {
            region, permission, ..
        } = &prepared.source
        else {
            return Err(Failure::Integrity);
        };
        let operation = argument.operation;
        if *permission != operation.access_mode() {
            return Err(Failure::Integrity);
        }
        let source = match (operation, &argument.action) {
            (ValueInputOperation::Read, ArgumentAction::ReserveRead(referent))
            | (ValueInputOperation::Edit, ArgumentAction::ReserveUpdate(referent)) => {
                if matches!(
                    region.root,
                    super::Root::Expression(_) | super::Root::Temporary { .. }
                ) {
                    return Err(Violation {
                        location: self.at,
                        rule: AccessRule::ExpiredRoot,
                        root: region.root,
                        fields: region.fields.clone(),
                        retained: None,
                    }
                    .into());
                }
                ActivationAccessSource::Existing(*referent)
            }
            (
                ValueInputOperation::Read | ValueInputOperation::Edit,
                ArgumentAction::AccessOwnedValue,
            ) if region.root == (super::Root::Temporary { call, index }) => {
                self.commit_temporary_input(
                    call,
                    index,
                    &Holder::Prepared { call, index }.place(),
                )?;
                ActivationAccessSource::Temporary { call, index }
            }
            _ => return Err(Failure::Integrity),
        };
        Ok(ActivationAccess {
            source,
            data,
            operation,
            region: region.clone(),
            created: self.at,
        })
    }

    fn commit_temporary_input(
        &mut self,
        call: AccessExpressionId,
        index: usize,
        from: &resources::Place,
    ) -> Result<()> {
        self.world.resources.move_to(
            self.typed.program(),
            from,
            &Holder::Temporary { call, index }.place(),
            false,
            &mut self.steps,
        )?;
        Ok(())
    }
    pub(super) fn commit_inputs(&mut self, call: AccessExpressionId) -> Result<()> {
        let arguments = self
            .typed
            .body()
            .call_arguments(call)
            .ok_or(Failure::Integrity)?;
        for (index, argument) in arguments.preparation_order() {
            let Some(source) = self.committed_input_source(call, index, argument)? else {
                continue;
            };
            if argument.operation.access_mode() == Mode::Own {
                self.world.resources.invoke(
                    self.typed.program(),
                    &source,
                    call,
                    index,
                    &mut self.steps,
                )?;
            } else {
                self.commit_temporary_input(call, index, &source)?;
            }
        }
        Ok(())
    }
    fn committed_input_source(
        &mut self,
        call: AccessExpressionId,
        index: usize,
        argument: &Argument,
    ) -> Result<Option<resources::Place>> {
        let prepared = Holder::Prepared { call, index };
        if let ArgumentAction::ReserveTransfer { place, adapters } = &argument.action {
            return self.materialize_input(*place, adapters, prepared).map(Some);
        }
        Ok(self
            .world
            .resources
            .contains(prepared)
            .then(|| prepared.place()))
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
                        Some(
                            nocter_declarations::value_access::PayloadSchema::Optional(_)
                                | nocter_declarations::value_access::PayloadSchema::Fallible(_)
                        )
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
