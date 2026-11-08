use super::super::liveness::Carrier;
use super::super::state::{Prepared, Value, World};
use super::super::{AccessRule, Creation, RetainedAccess, Violation};
use super::{Context, Failure, Result};
use crate::value_access::ExpressionKind;
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
impl Context<'_, '_> {
    pub(super) fn commit(&mut self, call: AccessExpressionId) -> Result<Vec<World>> {
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
        let prepared = (0..arguments.len())
            .map(|index| {
                self.world
                    .prepared
                    .get(&(call, index))
                    .cloned()
                    .ok_or(Failure::Integrity)
            })
            .collect::<Result<Vec<_>>>()?;
        self.check_bundle(call, &prepared)?;
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
            caps.push(match value {
                Prepared::Reserved {
                    region,
                    actor,
                    permission,
                    ..
                } => Some((region.clone(), *actor, *permission)),
                Prepared::Value(value) => self
                    .base_value(*value)?
                    .map(|(region, actor)| (region, actor, value.mode)),
            });
        }
        for (index, left) in caps.iter().enumerate() {
            if let Some((region, _, mode)) = left {
                for (other, right) in caps.iter().enumerate().skip(index + 1) {
                    if let Some((other_region, _, other_mode)) = right
                        && region.overlaps(other_region)
                        && (*mode != Mode::Read || *other_mode != Mode::Read)
                    {
                        let created = match &prepared[other] {
                            Prepared::Reserved { at, .. } => Creation::Action(*at),
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
                            root: region.root,
                            fields: region.fields.clone(),
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
        for (region, actor, permission) in caps.iter().flatten() {
            if *permission == Mode::Read {
                self.world
                    .check(region, *actor, *permission, self.at, self.live)?;
            } else {
                self.write(region, *actor, *permission, Some(call))?;
            }
        }
        Ok(())
    }
}
