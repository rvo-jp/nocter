mod call;
mod ownership;
use super::super::{IntegrityError, Location, Operation};
use super::{
    AccessRule, Creation, Liveness, Root, Violation,
    liveness::Carrier,
    state::{Key, Node, Prepared, Staged, Value, World},
};
use crate::value_access::{
    ArgumentAction, BoundaryAction, ExpressionKind, Query as TypedQuery, ValueType,
};
use nocter_declarations::value_access::ResultReferent;
use nocter_model::{AccessExpressionId, AccessLocalId, ValueAccessMode as Mode};

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
) -> Result<Vec<(World, Vec<super::super::resources::Step>)>> {
    let mut context = Context {
        typed,
        world,
        at,
        live,
        steps: Vec::new(),
    };
    context.ownership_before(operation)?;
    match operation {
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
            context.world.values.insert(Carrier::Local(*local), value);
        }
        Operation::Assign { target, value } => {
            let replacement = context.world.value(*value)?;
            if replacement.mode != Mode::Own {
                return Err(Failure::Integrity);
            }
            let (root, actor) = context.base(*target)?.ok_or(Failure::Integrity)?;
            context.write(root, actor, Mode::Update, None)?;
            if context.local_mode(*target)? == Mode::Own {
                context
                    .world
                    .values
                    .insert(Carrier::Local(*target), replacement);
            }
        }
        Operation::CompoundAssign { target, value, .. } => {
            context.read(context.world.value(*value)?)?;
            let (root, actor) = context.base(*target)?.ok_or(Failure::Integrity)?;
            context.write(root, actor, Mode::Update, None)?;
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
    Ok(vec![(context.world.clone(), context.steps)])
}
impl Context<'_, '_> {
    fn end_evaluation_access(&self, id: AccessExpressionId) -> Result<()> {
        for holder in self.world.resources.ending(id)? {
            if let super::super::resources::Holder::Temporary { call, index } = holder {
                self.write(Root::Temporary { call, index }, None, Mode::Own, None)?;
            }
        }

        Ok(())
    }
    fn apply_call(
        &mut self,
        call: AccessExpressionId,
    ) -> Result<Vec<(World, Vec<super::super::resources::Step>)>> {
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
            next.ownership_after(&Operation::CommitCall(call))?;
            let steps = next.steps;
            results.push((world, steps));
        }
        Ok(results)
    }
    fn stage_return(&mut self, expression: AccessExpressionId) -> Result<()> {
        let value = self.world.value(expression)?;
        self.read(value)?;
        if let Some(key) = value.access {
            let node = self.world.nodes.get(&key).ok_or(Failure::Integrity)?;
            if value.mode == Mode::Update && node.permission != Mode::Update {
                return Err(Failure::Integrity);
            }
            let root = node.root;
            let declaration = self
                .typed
                .program()
                .callables()
                .get(self.typed.callable())
                .ok_or(Failure::Integrity)?;
            let allowed = match (root, declaration.result_referent()) {
                (Root::Input(parameter), ResultReferent::Inputs(inputs)) => {
                    inputs.contains(&parameter)
                }
                _ => false,
            };
            if !allowed {
                return Err(Violation {
                    location: self.at,
                    rule: AccessRule::InvalidReturnRoot,
                    root,
                    retained: None,
                }
                .into());
            }
        }
        self.world.staged = Some(Staged { value, at: self.at });

        Ok(())
    }

    fn local_mode(&self, local: AccessLocalId) -> Result<Mode> {
        Ok(self
            .typed
            .body()
            .locals()
            .get(local)
            .ok_or(Failure::Integrity)?
            .ty
            .mode())
    }
    fn base(&self, local: AccessLocalId) -> Result<Option<(Root, Option<Key>)>> {
        if self.local_mode(local)? == Mode::Own {
            return Ok(Some((Root::Owned(local), None)));
        }
        let value = self.world.local(self.typed, local)?;
        self.base_value(value)
    }
    fn base_value(&self, value: Value) -> Result<Option<(Root, Option<Key>)>> {
        value
            .access
            .map(|key| {
                Ok((
                    self.world.nodes.get(&key).ok_or(Failure::Integrity)?.root,
                    Some(key),
                ))
            })
            .transpose()
    }
    fn read(&self, value: Value) -> Result<()> {
        if let Some((root, actor)) = self.base_value(value)? {
            self.world
                .check(root, actor, Mode::Read, self.at, self.live)?;
        }
        Ok(())
    }
    fn write(
        &self,
        root: Root,
        actor: Option<Key>,
        mode: Mode,
        except: Option<AccessExpressionId>,
    ) -> Result<()> {
        self.world.check_reserved(root, self.at, except)?;
        // Complete assignment can restore an absent X root. Its old aliases cannot survive a
        // preceding valid destruction, but a reservation still protects the slot's identity.
        if !self.world.alive(root) && matches!(root, Root::Owned(_)) && mode == Mode::Update {
            return Ok(());
        }
        if mode == Mode::Update
            && actor.is_some_and(|key| {
                self.world
                    .nodes
                    .get(&key)
                    .is_none_or(|node| node.permission != Mode::Update)
            })
        {
            return Err(Failure::Integrity);
        }
        self.world.check(root, actor, mode, self.at, self.live)?;
        Ok(())
    }
    fn destroy(&mut self, local: AccessLocalId) -> Result<()> {
        self.write(Root::Owned(local), None, Mode::Own, None)?;
        self.world.values.remove(&Carrier::Local(local));
        Ok(())
    }
    fn create(&mut self, key: Key, base: Option<(Root, Option<Key>)>, mode: Mode) -> Result<Value> {
        let Some((root, parent)) = base else {
            if mode == Mode::Update {
                return Err(Failure::Integrity);
            }
            return Ok(Value::without_referent(mode));
        };
        if self.world.nodes.contains_key(&key) {
            return Err(Failure::Integrity);
        }
        if let Some(parent) = parent
            && mode == Mode::Update
            && self
                .world
                .nodes
                .get(&parent)
                .ok_or(Failure::Integrity)?
                .permission
                == Mode::Read
        {
            return Err(Failure::Integrity);
        }
        self.world.check(root, parent, mode, self.at, self.live)?;
        self.world.nodes.insert(
            key,
            Node {
                root,
                parent,
                permission: mode,
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
        let ValueType::Value(ty) = expression.ty else {
            return Err(Failure::Integrity);
        };
        match expression.kind {
            ExpressionKind::Place(local) => {
                self.create(Key::Expression(id), self.base(local)?, Mode::Read)
            }
            ExpressionKind::Update(local) => {
                self.create(Key::Expression(id), self.base(local)?, Mode::Update)
            }
            ExpressionKind::Transfer(local) => {
                self.destroy(local)?;
                Ok(Value::without_referent(Mode::Own))
            }
            ExpressionKind::Boundary { source, action } => {
                let place = match self
                    .typed
                    .body()
                    .expressions()
                    .get(source)
                    .ok_or(Failure::Integrity)?
                    .kind
                {
                    ExpressionKind::Place(local) => Some(local),
                    _ => None,
                };
                match action {
                    BoundaryAction::ReadPlace | BoundaryAction::DelegateUpdate => self.create(
                        Key::Expression(id),
                        self.base(place.ok_or(Failure::Integrity)?)?,
                        ty.mode(),
                    ),
                    BoundaryAction::ReadAccess => self.create(
                        Key::Expression(id),
                        self.base_value(self.world.value(source)?)?,
                        Mode::Read,
                    ),
                    BoundaryAction::CopyPayload => {
                        if let Some(local) = place {
                            if let Some((root, actor)) = self.base(local)? {
                                self.world
                                    .check(root, actor, Mode::Read, self.at, self.live)?;
                            }
                        } else {
                            self.read(self.world.value(source)?)?;
                        }
                        Ok(Value::without_referent(ty.mode()))
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
            ExpressionKind::Integer(_)
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
            ArgumentAction::ReserveRead(local)
            | ArgumentAction::ReserveUpdate(local)
            | ArgumentAction::ReserveTransfer(local) => {
                let mode = match argument.action {
                    ArgumentAction::ReserveRead(_) => Mode::Read,
                    ArgumentAction::ReserveUpdate(_) => Mode::Update,
                    _ => Mode::Own,
                };
                if let Some((root, actor)) = self.base(local)? {
                    if !self.world.alive(root) {
                        return Err(Violation {
                            location: self.at,
                            rule: AccessRule::ExpiredRoot,
                            root,
                            retained: None,
                        }
                        .into());
                    }
                    Prepared::Reserved {
                        root,
                        actor,
                        permission: mode,
                        at: self.at,
                    }
                } else {
                    Prepared::Value(Value::without_referent(mode))
                }
            }
            ArgumentAction::CopyPayload => {
                self.read(self.world.value(argument.value)?)?;
                Prepared::Value(Value::without_referent(Mode::Own))
            }
            ArgumentAction::ReadAccessValue => {
                let base = self.base_value(self.world.value(argument.value)?)?;
                Prepared::Value(self.create(Key::Argument(call, index), base, Mode::Read)?)
            }
            ArgumentAction::ReadOwnedValue => Prepared::Reserved {
                root: Root::Temporary { call, index },
                actor: None,
                permission: Mode::Read,
                at: self.at,
            },
            ArgumentAction::ReadValue | ArgumentAction::UpdateValue | ArgumentAction::TakeValue => {
                Prepared::Value(self.world.value(argument.value)?)
            }
            ArgumentAction::Interrupted => return Err(Failure::Integrity),
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
