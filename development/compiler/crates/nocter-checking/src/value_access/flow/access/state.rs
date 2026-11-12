use super::super::{IntegrityError, Location};
use super::{
    AccessRule, Creation, Liveness, Region, RetainedAccess, RetentionKind, Root, Violation,
    liveness::{Carrier, Live},
};
use crate::value_access::Query as TypedQuery;
use nocter_model::{AccessExpressionId, AccessLocalId, ValueAccessMode as Mode};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Key {
    Entry(AccessLocalId),
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
    pub kind: RetentionKind,
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
            if declaration.origin.initialized_on_entry() {
                let mode = declaration.access().mode();
                let value = if mode == Mode::Own {
                    Value::without_referent(mode)
                } else {
                    let key = Key::Entry(local);
                    world.nodes.insert(
                        key,
                        Node {
                            region: match declaration.origin {
                                crate::value_access::LocalOrigin::Static(id) => Root::Static(id),
                                _ => Root::Input(local),
                            }
                            .region(),
                            parent: None,
                            kind: RetentionKind::Access(mode),
                            created: Creation::Entry(local),
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
            Root::Input(_) | Root::Static(_) => true,
            Root::Temporary { call, index } => {
                use super::super::resources::Holder;
                self.resources.contains(Holder::Prepared { call, index })
                    || self.resources.contains(Holder::Temporary { call, index })
            }
        }
    }
    pub fn writable_actor(&self, mut actor: Option<Key>) -> Result<bool, IntegrityError> {
        while let Some(key) = actor {
            let node = self.nodes.get(&key).ok_or(INVALID)?;
            match node.kind {
                RetentionKind::Place | RetentionKind::Tag => actor = node.parent,
                RetentionKind::Access(mode) => return Ok(mode == Mode::Update),
            }
        }
        Ok(true)
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
        self.retain_tags(&mut retained)?;
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
        // Update demand through a retained projection also demands its existing authority.
        // Dead neutral ancestors carry no remaining identity demand. Retain the first actual
        // authority grant, and let reparenting compress neutral intermediates without upgrading it.
        let mut pending: Vec<_> = updating.iter().copied().collect();
        while let Some(key) = pending.pop() {
            let node = self.nodes.get(&key).ok_or(INVALID)?;
            if matches!(node.kind, RetentionKind::Place | RetentionKind::Tag)
                && let Some(parent) = node.parent
            {
                if matches!(
                    self.nodes.get(&parent).ok_or(INVALID)?.kind,
                    RetentionKind::Access(_)
                ) {
                    retained.insert(parent);
                }
                if updating.insert(parent) {
                    pending.push(parent);
                }
            }
        }
        let mut nodes = BTreeMap::new();
        for &key in &retained {
            let mut node = self.nodes.get(&key).ok_or(INVALID)?.clone();
            if !updating.contains(&key) && matches!(node.kind, RetentionKind::Access(_)) {
                node.kind = RetentionKind::Access(Mode::Read);
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
    fn retain_tags(&self, retained: &mut BTreeSet<Key>) -> Result<(), IntegrityError> {
        let mut roots: Vec<_> = retained.iter().copied().collect();
        // Even a read reservation which releases neutral place intermediates needs its tag pin.
        roots.extend(
            self.prepared
                .values()
                .filter_map(|prepared| match prepared {
                    Prepared::Reserved { actor, .. } => *actor,
                    Prepared::Value(_) => None,
                }),
        );
        let mut visited = BTreeSet::new();
        for root in roots {
            let mut current = Some(root);
            while let Some(key) = current {
                if !visited.insert(key) {
                    break;
                }
                let node = self.nodes.get(&key).ok_or(INVALID)?;
                if node.kind == RetentionKind::Tag {
                    retained.insert(key);
                }
                current = node.parent;
            }
        }
        Ok(())
    }
    fn witness(&self, key: Key, location: Location, live: &Liveness) -> Option<Location> {
        for (&carrier, value) in &self.values {
            if value.access == Some(key)
                && let Some(at) = (if self
                    .nodes
                    .get(&key)
                    .is_some_and(|node| node.kind == RetentionKind::Access(Mode::Update))
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
    pub fn check_lifetime(&self, region: &Region, location: Location) -> Result<(), Violation> {
        if self.alive(region.root) {
            return Ok(());
        }
        Err(Violation {
            location,
            rule: AccessRule::ExpiredRoot,
            root: region.root,
            fields: region.fields.clone(),
            retained: None,
        })
    }
    pub fn check(
        &self,
        region: &Region,
        actor: Option<Key>,
        permission: Mode,
        location: Location,
        live: &Liveness,
    ) -> Result<(), Violation> {
        self.check_excluding_places(region, actor, permission, location, live, &BTreeSet::new())
    }
    /// A validated atomic bundle already checks its frozen places against each other. Only
    /// those neutral place nodes may be exempt here; existing read/edit grants never are.
    pub fn check_excluding_places(
        &self,
        region: &Region,
        actor: Option<Key>,
        permission: Mode,
        location: Location,
        live: &Liveness,
        places: &BTreeSet<Key>,
    ) -> Result<(), Violation> {
        self.check_region(region, actor, permission, location, live, places, false)
    }
    pub fn check_tag(
        &self,
        region: &Region,
        actor: Option<Key>,
        location: Location,
        live: &Liveness,
    ) -> Result<(), Violation> {
        self.check_region(
            region,
            actor,
            Mode::Read,
            location,
            live,
            &BTreeSet::new(),
            true,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn check_region(
        &self,
        region: &Region,
        actor: Option<Key>,
        permission: Mode,
        location: Location,
        live: &Liveness,
        places: &BTreeSet<Key>,
        tag_only: bool,
    ) -> Result<(), Violation> {
        self.check_lifetime(region, location)?;
        let mut ancestors = BTreeSet::new();
        let mut cursor = actor;
        while let Some(key) = cursor {
            ancestors.insert(key);
            cursor = self.nodes.get(&key).and_then(|node| node.parent);
        }
        for (&key, node) in &self.nodes {
            if !node.region.overlaps(region) || ancestors.contains(&key) {
                continue;
            }
            if tag_only && node.region.inside_payload(region) {
                continue;
            }
            if node.kind == RetentionKind::Tag && region.inside_payload(&node.region) {
                continue;
            }
            if node.kind == RetentionKind::Place && places.contains(&key) {
                continue;
            }
            if permission != Mode::Read || node.kind == RetentionKind::Access(Mode::Update) {
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
                        kind: node.kind,
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
                        kind: RetentionKind::Access(*permission),
                        required: Some(location),
                    }),
                });
            }
        }
        Ok(())
    }
}
