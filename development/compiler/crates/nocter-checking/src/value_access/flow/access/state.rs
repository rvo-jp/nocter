use super::super::{IntegrityError, Location};
use super::{
    AccessRule, Creation, Liveness, Region, RetainedAccess, Root, Violation,
    liveness::{Carrier, Live},
};
use crate::value_access::{LocalOrigin, Query as TypedQuery};
use nocter_model::{AccessExpressionId, AccessLocalId, AccessParameterId, ValueAccessMode as Mode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Key {
    Parameter(AccessParameterId),
    Expression(AccessExpressionId),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Value {
    pub mode: Mode,
    pub access: Option<Key>,
}
impl Value {
    pub const fn without_referent(mode: Mode) -> Self {
        Self { mode, access: None }
    }
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Node {
    pub region: Region,
    pub parent: Option<Key>,
    pub permission: Mode,
    pub created: Creation,
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Prepared {
    Reserved {
        region: Region,
        actor: Option<Key>,
        permission: Mode,
        at: Location,
    },
    Value(Value),
}
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct World {
    pub resources: super::super::resources::State,
    pub values: BTreeMap<Carrier, Value>,
    pub nodes: BTreeMap<Key, Node>,
    pub prepared: BTreeMap<(AccessExpressionId, usize), Prepared>,
}
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;
impl World {
    pub fn entry(typed: &TypedQuery) -> Result<Self, IntegrityError> {
        let mut world = Self {
            resources: super::super::resources::State::entry(typed)?,
            values: BTreeMap::new(),
            nodes: BTreeMap::new(),
            prepared: BTreeMap::new(),
        };
        for (local, declaration) in typed.body().locals().iter() {
            if let LocalOrigin::Parameter(parameter) = declaration.origin {
                let mode = declaration.access().mode();
                let value = if mode == Mode::Own {
                    Value::without_referent(mode)
                } else {
                    let key = Key::Parameter(parameter);
                    world.nodes.insert(
                        key,
                        Node {
                            region: Root::Input(parameter).region(),
                            parent: None,
                            permission: mode,
                            created: Creation::Parameter(local),
                        },
                    );
                    Value {
                        mode,
                        access: Some(key),
                    }
                };
                if mode != Mode::Own {
                    world.values.insert(Carrier::Local(local), value);
                }
            }
        }
        Ok(world)
    }
    pub fn local(&self, typed: &TypedQuery, local: AccessLocalId) -> Result<Value, IntegrityError> {
        let mode = typed
            .body()
            .locals()
            .get(local)
            .ok_or(INVALID)?
            .access()
            .mode();
        if mode == Mode::Own {
            Ok(Value::without_referent(Mode::Own))
        } else {
            self.values
                .get(&Carrier::Local(local))
                .copied()
                .ok_or(INVALID)
        }
    }
    pub fn value(&self, expression: AccessExpressionId) -> Result<Value, IntegrityError> {
        self.values
            .get(&Carrier::Expression(expression))
            .copied()
            .ok_or(INVALID)
    }
    pub fn alive(&self, root: Root) -> bool {
        match root {
            Root::Owned(local) => self
                .resources
                .contains(super::super::resources::Holder::Local(local)),
            Root::Input(_) => true,
            Root::Temporary { call, index } => {
                use super::super::resources::Holder;
                self.resources.contains(Holder::Prepared { call, index })
                    || self.resources.contains(Holder::Temporary { call, index })
            }
        }
    }
    pub fn trim(&mut self, live: Option<&Live>) -> Result<(), IntegrityError> {
        self.values
            .retain(|carrier, _| live.is_some_and(|live| live.contains_key(carrier)));
        self.prepared.retain(|&(call, index), _| {
            live.is_some_and(|live| live.contains_key(&Carrier::Argument { call, index }))
        });
        // Releasing a prepared access on cancellation does not dereference its root.
        // The prefix remains present for cleanup; reservations continue to protect identity.
        for (&(call, index), prepared) in &mut self.prepared {
            let demand = live.and_then(|live| live.get(&Carrier::Argument { call, index }));
            if demand.is_some_and(|demand| demand.uses.is_empty())
                && let Prepared::Value(value) = prepared
            {
                value.access = None;
            }
        }
        let mut retained = self
            .values
            .values()
            .filter_map(|value| value.access)
            .collect::<BTreeSet<_>>();
        for prepared in self.prepared.values() {
            match prepared {
                Prepared::Reserved {
                    actor, permission, ..
                } if *permission != Mode::Read => retained.extend(actor),
                Prepared::Reserved { .. } => {}
                Prepared::Value(value) => retained.extend(value.access),
            }
        }
        let mut updating = BTreeSet::new();
        if let Some(live) = live {
            for (&carrier, value) in &self.values {
                if live
                    .get(&carrier)
                    .is_some_and(|demand| !demand.updates.is_empty())
                {
                    updating.extend(value.access);
                }
            }
        }
        for (&(call, index), prepared) in &self.prepared {
            if live
                .and_then(|live| live.get(&Carrier::Argument { call, index }))
                .is_some_and(|demand| !demand.updates.is_empty())
            {
                match prepared {
                    Prepared::Reserved { actor, .. } => updating.extend(actor),
                    Prepared::Value(value) => updating.extend(value.access),
                }
            }
        }
        let mut nodes = BTreeMap::new();
        for &key in &retained {
            let mut node = self.nodes.get(&key).ok_or(INVALID)?.clone();
            if !updating.contains(&key) {
                node.permission = Mode::Read;
            }
            while let Some(parent) = node.parent {
                if retained.contains(&parent) {
                    break;
                }
                node.parent = self.nodes.get(&parent).ok_or(INVALID)?.parent;
            }
            nodes.insert(key, node);
        }
        // A pending I request needs read authority and identity protection, not an otherwise
        // dead U intermediary. Reparent the reservation without activating an input early.
        for prepared in self.prepared.values_mut() {
            if let Prepared::Reserved { actor, .. } = prepared {
                while let Some(key) = *actor {
                    if retained.contains(&key) {
                        break;
                    }
                    *actor = self.nodes.get(&key).ok_or(INVALID)?.parent;
                }
            }
        }
        self.nodes = nodes;
        Ok(())
    }
    fn witness(&self, key: Key, location: Location, live: &Liveness) -> Option<Location> {
        for (&carrier, value) in &self.values {
            if value.access == Some(key)
                && let Some(at) = (if self
                    .nodes
                    .get(&key)
                    .is_some_and(|node| node.permission == Mode::Update)
                {
                    live.required_update(location, carrier)
                } else {
                    None
                })
                .or_else(|| live.required_use(location, carrier))
            {
                return Some(at);
            }
        }
        for (&(call, index), prepared) in &self.prepared {
            let actor = match prepared {
                Prepared::Reserved { actor, .. } => *actor,
                Prepared::Value(value) => value.access,
            };
            if actor == Some(key)
                && let Some(at) = live
                    .required_update(location, Carrier::Argument { call, index })
                    .or_else(|| live.required_use(location, Carrier::Argument { call, index }))
            {
                return Some(at);
            }
        }
        None
    }
    pub fn check(
        &self,
        region: &Region,
        actor: Option<Key>,
        permission: Mode,
        location: Location,
        live: &Liveness,
    ) -> Result<(), Violation> {
        if !self.alive(region.root) {
            return Err(Violation {
                location,
                rule: AccessRule::ExpiredRoot,
                root: region.root,
                fields: region.fields.clone(),
                retained: None,
            });
        }
        let mut ancestors = BTreeSet::new();
        let mut cursor = actor;
        while let Some(key) = cursor {
            ancestors.insert(key);
            cursor = self.nodes.get(&key).and_then(|node| node.parent);
        }
        for (&key, node) in &self.nodes {
            if node.region.overlaps(region)
                && !ancestors.contains(&key)
                && (permission != Mode::Read || node.permission == Mode::Update)
            {
                return Err(Violation {
                    location,
                    rule: match permission {
                        Mode::Read => AccessRule::ConflictingRead,
                        Mode::Update => AccessRule::ConflictingUpdate,
                        Mode::Own => AccessRule::ConflictingTransfer,
                    },
                    root: region.root,
                    fields: region.fields.clone(),
                    retained: Some(RetainedAccess {
                        created: node.created,
                        permission: node.permission,
                        required: self.witness(key, location, live),
                    }),
                });
            }
        }
        Ok(())
    }
    pub fn check_reserved(
        &self,
        region: &Region,
        location: Location,
        except: Option<AccessExpressionId>,
    ) -> Result<(), Violation> {
        for (&(call, _), prepared) in &self.prepared {
            if Some(call) == except {
                continue;
            }
            if let Prepared::Reserved {
                region: reserved,
                permission,
                at,
                ..
            } = prepared
                && reserved.overlaps(region)
            {
                return Err(Violation {
                    location,
                    rule: AccessRule::ReservedModification,
                    root: region.root,
                    fields: region.fields.clone(),
                    retained: Some(RetainedAccess {
                        created: Creation::Action(*at),
                        permission: *permission,
                        required: Some(location),
                    }),
                });
            }
        }
        Ok(())
    }
}
