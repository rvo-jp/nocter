use super::super::liveness::Carrier;
use super::super::state::{Prepared, Value, World};
use super::super::{AccessRule, Creation, RetainedAccess, RetentionKind, Violation};
use super::{Context, Failure, Result};
use crate::value_access::{ArgumentAction, ExpressionKind, Referent};
use nocter_model::{AccessExpressionId, ValueAccessMode as Mode};
use std::collections::BTreeSet;
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
                        && !self.disjoint_prefix_arguments(call, index, other)?
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
                                kind: RetentionKind::Access(*other_mode),
                                required: Some(self.at),
                            }),
                        }
                        .into());
                    }
                }
            }
        }
        let places: BTreeSet<_> = caps
            .iter()
            .flatten()
            .filter_map(|(_, actor, _)| *actor)
            .collect();
        for (region, actor, permission) in caps.iter().flatten() {
            if *permission != Mode::Read {
                self.world.check_reserved(region, self.at, Some(call))?;
                if *permission == Mode::Update && !self.world.writable_actor(*actor)? {
                    return Err(Failure::Integrity);
                }
            }
            self.world.check_excluding_places(
                region,
                *actor,
                *permission,
                self.at,
                self.live,
                &places,
            )?;
        }
        Ok(())
    }
    fn disjoint_prefix_arguments(
        &self,
        call: AccessExpressionId,
        left: usize,
        right: usize,
    ) -> Result<bool> {
        let ExpressionKind::Call { ref arguments, .. } = self
            .typed
            .body()
            .expressions()
            .get(call)
            .ok_or(Failure::Integrity)?
            .kind
        else {
            return Err(Failure::Integrity);
        };
        let selection = |index: usize| match &arguments.get(index)?.action {
            ArgumentAction::ReserveTransfer {
                place: Referent::Evaluated(id),
                ..
            } => Some(*id),
            _ => None,
        };
        let Some((left, right)) = selection(left).zip(selection(right)) else {
            return Ok(false);
        };
        let Some(frontier) = self.numeric.prefix(self.at, left) else {
            return Ok(false);
        };
        // Both ordered extension proofs belong to this atomic operation and one array epoch:
        // each advances its predecessor by one. Distinct source keys alone prove nothing.
        Ok(left != right && self.numeric.prefix(self.at, right) == Some(frontier))
    }
}
