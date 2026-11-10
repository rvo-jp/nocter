mod call;
mod ownership;
mod selection;
use super::super::resources::StepPlan;
use super::super::{IntegrityError, Location, Operation};
use super::{
    AccessRule, Creation, Liveness, Region, RetentionKind, Root, Violation,
    liveness::Carrier,
    state::{Key, Node, Prepared, Value, World},
};
use crate::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Query as TypedQuery, Referent, SelectionAction,
};
use nocter_model::{AccessExpressionId, AccessLocalId, AccessPlaceId, ValueAccessMode as Mode};

pub(super) enum Failure {
    Integrity,
    Violation(Violation),
}
impl From<IntegrityError> for Failure {
    fn from(_: IntegrityError) -> Self {
        Self::Integrity
    }
}
impl From<Violation> for Failure {
    fn from(value: Violation) -> Self {
        Self::Violation(value)
    }
}
type Result<T> = std::result::Result<T, Failure>;
struct Context<'a, 'b> {
    typed: &'a TypedQuery,
    world: &'b mut World,
    at: Location,
    live: &'a Liveness,
    steps: Vec<super::super::resources::Step>,
}

pub(super) fn apply(
    typed: &TypedQuery,
    world: &mut World,
    operation: &Operation,
    at: Location,
    live: &Liveness,
) -> Result<Vec<(World, StepPlan)>> {
    let mut context = Context {
        typed,
        world,
        at,
        live,
        steps: Vec::new(),
    };
    context.ownership_before(operation)?;
    let before = context.steps.len();
    let operation_state = context.world.resources.condition();
    match operation {
        Operation::IndexFailure(id) => context.index_failure(*id)?,
        Operation::BeginEvaluation(_) => {}
        Operation::EndEvaluation(id) => context.end_evaluation_access(*id)?,
        Operation::Evaluate(id) => {
            let value = context.evaluate(*id)?;
            context.world.values.insert(Carrier::Expression(*id), value);
        }
        Operation::PrepareOperand { value, .. } | Operation::Condition(value) => {
            context.read(context.world.value(*value)?)?;
        }
        Operation::Forward { source, result } => {
            let value = context.world.value(*source)?;
            context
                .world
                .values
                .insert(Carrier::Expression(*result), value);
        }
        Operation::BooleanAnswer { source, result } => {
            context.read(context.world.value(*source)?)?;
            context.world.values.insert(
                Carrier::Expression(*result),
                Value::without_referent(Mode::Own),
            );
        }
        Operation::BooleanConstant { result, .. } => {
            context.world.values.insert(
                Carrier::Expression(*result),
                Value::without_referent(Mode::Own),
            );
        }
        Operation::Bind { local, value } => {
            let value = context.world.value(*value)?;
            if context.local_mode(*local)? != Mode::Own {
                context.world.values.insert(Carrier::Local(*local), value);
            }
        }
        Operation::Assign { target, value } => {
            let replacement = context.world.value(*value)?;
            if replacement.mode != Mode::Own {
                return Err(Failure::Integrity);
            }
            let (region, actor) = context.referent(*target)?.ok_or(Failure::Integrity)?;
            context.write(&region, actor, Mode::Update, None)?;
        }
        Operation::CompoundAssign { target, value, .. } => {
            context.read(context.world.value(*value)?)?;
            let (region, actor) = context.referent(*target)?.ok_or(Failure::Integrity)?;
            context.write(&region, actor, Mode::Update, None)?;
        }
        Operation::Drop(local) => context.destroy(*local)?,
        Operation::PrepareArgument { call, argument } => context.prepare(*call, *argument)?,
        Operation::CommitCall(call) => return context.apply_call(*call),
        Operation::CancelCall { call, prepared } => {
            for index in 0..*prepared {
                context
                    .world
                    .prepared
                    .remove(&(*call, index))
                    .ok_or(Failure::Integrity)?;
            }
        }
        Operation::Discard(value) => context.read(context.world.value(*value)?)?,
        Operation::StageReturn(value) => context.stage_return(*value)?,
        Operation::ExitScope { locals, .. } => {
            for &local in locals {
                if context.local_mode(local)? == Mode::Own
                    && context.world.alive(Root::Owned(local))
                {
                    context.destroy(local)?;
                }
                context.world.values.remove(&Carrier::Local(local));
            }
        }
    }
    context.ownership_after(operation)?;
    Ok(vec![(
        context.world.clone(),
        StepPlan::new(context.steps, before, operation_state),
    )])
}
impl Context<'_, '_> {
    fn end_evaluation_access(&self, id: AccessExpressionId) -> Result<()> {
        for holder in self.world.resources.ending(id)? {
            if let super::super::resources::Holder::Temporary { call, index } = holder {
                self.write(
                    &Root::Temporary { call, index }.region(),
                    None,
                    Mode::Own,
                    None,
                )?;
            }
        }

        Ok(())
    }
    fn apply_call(&mut self, call: AccessExpressionId) -> Result<Vec<(World, StepPlan)>> {
        let alternatives = self.commit(call)?;
        let mut results = Vec::new();
        for mut world in alternatives {
            let mut next = Context {
                typed: self.typed,
                world: &mut world,
                at: self.at,
                live: self.live,
                steps: self.steps.clone(),
            };
            next.commit_inputs(call)?;
            let before = next.steps.len();
            let operation_state = next.world.resources.condition();
            next.receive_result(call)?;
            let plan = StepPlan::new(next.steps, before, operation_state);
            results.push((world, plan));
        }
        Ok(results)
    }
    fn stage_return(&mut self, expression: AccessExpressionId) -> Result<()> {
        let value = self.world.value(expression)?;
        self.read(value)?;
        if value.access.is_some() || value.mode != Mode::Own {
            return Err(Failure::Integrity);
        }
        Ok(())
    }

    fn local_mode(&self, local: AccessLocalId) -> Result<Mode> {
        Ok(self
            .typed
            .body()
            .locals()
            .get(local)
            .ok_or(Failure::Integrity)?
            .access()
            .mode())
    }
    fn base(&self, place: AccessPlaceId) -> Result<Option<(Region, Option<Key>)>> {
        let selected = self
            .typed
            .body()
            .places()
            .get(place)
            .ok_or(Failure::Integrity)?;
        Ok(self.base_local(selected.local)?.map(|(mut region, actor)| {
            region.project(&selected.fields);
            (region, actor)
        }))
    }
    fn base_local(&self, local: AccessLocalId) -> Result<Option<(Region, Option<Key>)>> {
        if self.local_mode(local)? == Mode::Own {
            return Ok(Some((Root::Owned(local).region(), None)));
        }
        self.base_value(self.world.local(self.typed, local)?)
    }
    fn resource_place(&self, place: AccessPlaceId) -> Result<super::super::resources::Place> {
        let selected = self
            .typed
            .body()
            .places()
            .get(place)
            .ok_or(Failure::Integrity)?;
        Ok(super::super::resources::Holder::Local(selected.local)
            .place()
            .project_path(&selected.fields))
    }
    fn referent(&self, source: Referent) -> Result<Option<(Region, Option<Key>)>> {
        match source {
            Referent::Place(place) => self.base(place),
            Referent::Evaluated(id) if self.typed.body().referent_data(source).is_some() => {
                self.base_value(self.world.value(id)?)
            }
            Referent::Evaluated(_) => Err(Failure::Integrity),
        }
    }
    fn base_value(&self, value: Value) -> Result<Option<(Region, Option<Key>)>> {
        value
            .access
            .map(|key| {
                Ok((
                    self.world
                        .nodes
                        .get(&key)
                        .ok_or(Failure::Integrity)?
                        .region
                        .clone(),
                    Some(key),
                ))
            })
            .transpose()
    }
    fn read(&self, value: Value) -> Result<()> {
        if let Some((region, actor)) = self.base_value(value)? {
            self.world
                .check(&region, actor, Mode::Read, self.at, self.live)?;
        }
        Ok(())
    }
    fn write(
        &self,
        region: &Region,
        actor: Option<Key>,
        mode: Mode,
        except: Option<AccessExpressionId>,
    ) -> Result<()> {
        self.world.check_reserved(region, self.at, except)?;
        // Complete assignment can restore an absent X region. Its old aliases cannot survive a
        // preceding valid destruction, but a reservation still protects the slot's identity.
        if !self.world.alive(region.root)
            && matches!(region.root, Root::Owned(_))
            && region.fields.is_root()
            && mode == Mode::Update
        {
            return Ok(());
        }
        if mode == Mode::Update && !self.world.writable_actor(actor)? {
            return Err(Failure::Integrity);
        }
        self.world.check(region, actor, mode, self.at, self.live)?;
        Ok(())
    }
    fn destroy(&mut self, local: AccessLocalId) -> Result<()> {
        self.write(&Root::Owned(local).region(), None, Mode::Own, None)?;
        self.world.values.remove(&Carrier::Local(local));
        Ok(())
    }
    fn create(
        &mut self,
        key: Key,
        base: Option<(Region, Option<Key>)>,
        mode: Mode,
    ) -> Result<Value> {
        self.create_node(key, base, RetentionKind::Access(mode))
    }
    fn retain_place(&mut self, key: Key, base: Option<(Region, Option<Key>)>) -> Result<Value> {
        self.create_node(key, base, RetentionKind::Place)
    }
    fn create_node(
        &mut self,
        key: Key,
        base: Option<(Region, Option<Key>)>,
        kind: RetentionKind,
    ) -> Result<Value> {
        let mode = match kind {
            RetentionKind::Place => Mode::Read,
            RetentionKind::Access(mode) => mode,
        };
        let Some((region, parent)) = base else {
            if mode == Mode::Update {
                return Err(Failure::Integrity);
            }
            return Ok(Value::without_referent(mode));
        };
        if self.world.nodes.contains_key(&key) {
            return Err(Failure::Integrity);
        }
        if kind == RetentionKind::Access(Mode::Update) && !self.world.writable_actor(parent)? {
            return Err(Failure::Integrity);
        }
        if kind == RetentionKind::Place {
            self.world.check_lifetime(&region, self.at)?;
        } else {
            self.world
                .check(&region, parent, mode, self.at, self.live)?;
        }
        self.world.nodes.insert(
            key,
            Node {
                region,
                parent,
                kind,
                created: Creation::Action(self.at),
            },
        );
        Ok(Value {
            mode,
            access: Some(key),
        })
    }
    fn evaluate(&mut self, id: AccessExpressionId) -> Result<Value> {
        let expression = self
            .typed
            .body()
            .expressions()
            .get(id)
            .ok_or(Failure::Integrity)?;
        let Some(ty) = expression.ty.normal() else {
            return Err(Failure::Integrity);
        };
        match expression.kind {
            ExpressionKind::Place(place) => {
                self.retain_place(Key::Expression(id), self.base(place)?)
            }
            ExpressionKind::Index {
                base,
                index,
                action,
                ..
            } => self.index(id, base, index, action),
            ExpressionKind::Select {
                source,
                ref fields,
                action: SelectionAction::Access,
            } => self.select(id, source, fields),
            ExpressionKind::Move(place) => {
                let (region, actor) = self.base(place)?.ok_or(Failure::Integrity)?;
                self.write(&region, actor, Mode::Own, None)?;
                Ok(Value::without_referent(Mode::Own))
            }
            ExpressionKind::Boundary { source, action } => {
                let place = self
                    .typed
                    .body()
                    .expressions()
                    .get(source)
                    .ok_or(Failure::Integrity)?
                    .referent(source);
                match action {
                    BoundaryAction::ReadPlace | BoundaryAction::DelegateUpdate => self.create(
                        Key::Expression(id),
                        self.referent(place.ok_or(Failure::Integrity)?)?,
                        ty.mode(),
                    ),
                    BoundaryAction::CopyPayload => {
                        if let Some(place) = place {
                            if let Some((region, actor)) = self.referent(place)? {
                                self.world
                                    .check(&region, actor, Mode::Read, self.at, self.live)?;
                            }
                        } else {
                            self.read(self.world.value(source)?)?;
                        }
                        Ok(Value::without_referent(ty.mode()))
                    }
                    BoundaryAction::PendingValue | BoundaryAction::Unproduced => {
                        Err(Failure::Integrity)
                    }
                    BoundaryAction::TakeValue => {
                        let value = self.world.value(source)?;
                        if value.mode != Mode::Own {
                            return Err(Failure::Integrity);
                        }
                        Ok(value)
                    }
                }
            }
            ExpressionKind::Function(_)
            | ExpressionKind::Closure { .. }
            | ExpressionKind::AdaptFunction { .. }
            | ExpressionKind::Select {
                action: SelectionAction::CopyPayload | SelectionAction::MovePayload,
                ..
            }
            | ExpressionKind::Aggregate { .. }
            | ExpressionKind::Repeat { .. }
            | ExpressionKind::Constant(_)
            | ExpressionKind::Integer(_)
            | ExpressionKind::Boolean(_)
            | ExpressionKind::Unit
            | ExpressionKind::Unary { .. }
            | ExpressionKind::Binary { .. } => Ok(Value::without_referent(Mode::Own)),
            ExpressionKind::If { .. } | ExpressionKind::Call { .. } => Err(Failure::Integrity),
        }
    }
    fn prepare(&mut self, call: AccessExpressionId, index: usize) -> Result<()> {
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
        let prepared = match argument.action {
            ArgumentAction::ReserveRead(place)
            | ArgumentAction::ReserveUpdate(place)
            | ArgumentAction::ReserveTransfer { place, .. } => {
                let mode = match argument.action {
                    ArgumentAction::ReserveRead(_) => Mode::Read,
                    ArgumentAction::ReserveUpdate(_) => Mode::Update,
                    _ => Mode::Own,
                };
                if let Some((region, actor)) = self.referent(place)? {
                    if !self.world.alive(region.root) {
                        return Err(Violation {
                            location: self.at,
                            rule: AccessRule::ExpiredRoot,
                            root: region.root,
                            fields: region.fields.clone(),
                            retained: None,
                        }
                        .into());
                    }
                    Prepared::Reserved {
                        region,
                        actor,
                        permission: mode,
                        at: self.at,
                    }
                } else {
                    Prepared::Value(Value::without_referent(mode))
                }
            }
            ArgumentAction::CopyPlace(place) => {
                if let Some((region, actor)) = self.referent(place)? {
                    self.world
                        .check(&region, actor, Mode::Read, self.at, self.live)?;
                }
                Prepared::Value(Value::without_referent(Mode::Own))
            }
            ArgumentAction::AccessOwnedValue => Prepared::Reserved {
                region: Root::Temporary { call, index }.region(),
                actor: None,
                permission: argument.operation.access_mode(),
                at: self.at,
            },
            ArgumentAction::TakeValue => Prepared::Value(self.world.value(argument.value)?),
            ArgumentAction::PendingValue(_)
            | ArgumentAction::Unproduced
            | ArgumentAction::Interrupted => {
                return Err(Failure::Integrity);
            }
        };
        if self
            .world
            .prepared
            .insert((call, index), prepared)
            .is_some()
        {
            return Err(Failure::Integrity);
        }
        Ok(())
    }
}
