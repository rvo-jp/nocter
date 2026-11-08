use super::super::liveness::Carrier;
use super::super::state::{Prepared, Value, World};
use super::super::{AccessRule, Creation, RetainedAccess, Violation};
use super::{Context, Failure, Result};
use crate::value_access::ExpressionKind;
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
        self.typed
            .program()
            .callables()
            .get(*target)
            .ok_or(Failure::Integrity)?;
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
        // The admitted payload contracts return values with no outside R/D/S. Abstract
        // invocation retains no argument access; it never invents a borrowed result.
        self.world.values.insert(
            Carrier::Expression(call),
            Value::without_referent(Mode::Own),
        );
        for index in 0..arguments.len() {
            self.world.prepared.remove(&(call, index));
        }
        Ok(vec![self.world.clone()])
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
                                required: Some(self.at),
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
