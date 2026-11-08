use super::super::liveness::Carrier;
use super::super::state::{Key, Prepared, Value, World};
use super::super::{AccessRule, Creation, RequiredUse, RetainedAccess, Root, Violation};
use super::{Context, Failure, Result};
use crate::value_access::ExpressionKind;
use nocter_declarations::value_access::ResultReferent;
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
impl Context<'_, '_> {
    pub(super) fn commit(&mut self, call: AccessExpressionId) -> Result<Vec<World>> {
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
        let result = declaration.result();
        let referents = declaration.result_referent().clone();
        let prepared = (0..arguments.len())
            .map(|index| {
                self.world
                    .prepared
                    .get(&(call, index))
                    .copied()
                    .ok_or(Failure::Integrity)
            })
            .collect::<Result<Vec<_>>>()?;
        self.check_bundle(call, &prepared)?;
        let mut inputs = Vec::new();
        for (index, prepared) in prepared.into_iter().enumerate() {
            let value = match prepared {
                Prepared::Reserved {
                    root,
                    permission: Mode::Own,
                    ..
                } => {
                    let Root::Owned(local) = root else {
                        return Err(Failure::Integrity);
                    };
                    self.world.roots.remove(&local);
                    Value::without_referent(Mode::Own)
                }
                Prepared::Reserved {
                    root,
                    actor,
                    permission,
                    ..
                } => self.create(Key::Argument(call, index), Some((root, actor)), permission)?,
                Prepared::Value(value) => value,
            };
            inputs.push(value);
        }
        let positions = self.result_positions(referents)?;
        let mut worlds = Vec::new();
        if let Some(positions) = positions {
            for position in positions {
                let input = *inputs.get(position).ok_or(Failure::Integrity)?;
                let mut alternative = self.world.clone();
                let mut context = Context {
                    typed: self.typed,
                    world: &mut alternative,
                    at: self.at,
                    live: self.live,
                };
                let base = context.base_value(input)?.ok_or(Failure::Integrity)?;
                let value = context.create(Key::Expression(call), Some(base), result.mode())?;
                context
                    .world
                    .values
                    .insert(Carrier::Expression(call), value);
                for index in 0..arguments.len() {
                    context.world.prepared.remove(&(call, index));
                }
                worlds.push(alternative);
            }
        } else {
            self.world.values.insert(
                Carrier::Expression(call),
                Value::without_referent(result.mode()),
            );
            for index in 0..arguments.len() {
                self.world.prepared.remove(&(call, index));
            }
            worlds.push(self.world.clone());
        }
        Ok(worlds)
    }
    fn result_positions(&self, referents: ResultReferent) -> Result<Option<Vec<usize>>> {
        Ok(match referents {
            ResultReferent::Value => None,
            ResultReferent::Inputs(parameters) => Some(
                parameters
                    .iter()
                    .map(|id| {
                        self.typed
                            .program()
                            .parameters()
                            .get(*id)
                            .map(|parameter| parameter.position())
                            .ok_or(Failure::Integrity)
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
        })
    }
    fn check_bundle(&self, call: AccessExpressionId, prepared: &[Prepared]) -> Result<()> {
        let mut caps = Vec::new();
        for value in prepared {
            caps.push(match *value {
                Prepared::Reserved {
                    root,
                    actor,
                    permission,
                    ..
                } => Some((root, actor, permission)),
                Prepared::Value(value) => self
                    .base_value(value)?
                    .map(|(root, actor)| (root, actor, value.mode)),
            });
        }
        for (index, left) in caps.iter().enumerate() {
            if let Some((root, _, mode)) = left {
                for (other, right) in caps.iter().enumerate().skip(index + 1) {
                    if let Some((other_root, _, other_mode)) = right
                        && root == other_root
                        && (*mode != Mode::Read || *other_mode != Mode::Read)
                    {
                        let created = match prepared[other] {
                            Prepared::Reserved { at, .. } => Creation::Action(at),
                            Prepared::Value(value) => {
                                self.world
                                    .nodes
                                    .get(&value.access.ok_or(Failure::Integrity)?)
                                    .ok_or(Failure::Integrity)?
                                    .created
                            }
                        };
                        return Err(Violation {
                            location: self.at,
                            rule: AccessRule::ConflictingArguments,
                            root: *root,
                            retained: Some(RetainedAccess {
                                created,
                                permission: *other_mode,
                                required: Some(RequiredUse::Action(self.at)),
                            }),
                        }
                        .into());
                    }
                }
            }
        }
        for &(root, actor, permission) in caps.iter().flatten() {
            if permission == Mode::Read {
                self.world
                    .check(root, actor, permission, self.at, self.live)?;
            } else {
                self.write(root, actor, permission, Some(call))?;
            }
        }
        Ok(())
    }
}
