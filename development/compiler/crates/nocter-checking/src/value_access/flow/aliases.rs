//! Derived lexical provenance. This index supplies neither access grants nor saved addresses.
use super::{IntegrityError, TypedQuery};
use crate::value_access::{BoundaryAction, ExpressionKind, Referent, Statement};
use nocter_model::{AccessLocalId, AccessPlaceId, ValueBindingKind};
use std::collections::{BTreeMap, BTreeSet};
const INVALID: IntegrityError = IntegrityError::InconsistentTypedBody;

#[derive(Clone, Debug, Default)]
pub(in crate::value_access) struct Aliases {
    sources: BTreeMap<AccessLocalId, Referent>,
}
impl Aliases {
    pub(in crate::value_access) fn prepare(typed: &TypedQuery) -> Result<Self, IntegrityError> {
        let mut sources = BTreeMap::new();
        for (_, statement) in typed.body().statements().iter() {
            let Statement::Bind { local, value } = *statement else {
                continue;
            };
            let kind = typed.body().locals().get(local).ok_or(INVALID)?.kind;
            let action = match kind {
                ValueBindingKind::ReadAlias => BoundaryAction::ReadPlace,
                ValueBindingKind::EditAlias => BoundaryAction::DelegateUpdate,
                _ => continue,
            };
            let ExpressionKind::Boundary {
                source,
                action: selected,
            } = typed.body().expressions().get(value).ok_or(INVALID)?.kind
            else {
                continue;
            };
            if selected != action {
                continue;
            }
            if let Some(referent) = typed
                .body()
                .expressions()
                .get(source)
                .ok_or(INVALID)?
                .referent(source)
                && sources.insert(local, referent).is_some()
            {
                return Err(INVALID);
            }
        }
        Ok(Self { sources })
    }
    /// Lexical roots for producer summaries, with all numeric selectors erased. These paths
    /// cannot supply saved addresses, index equality, access or definite array replacement.
    pub(in crate::value_access) fn producer_paths(
        &self,
        typed: &TypedQuery,
        source: Referent,
    ) -> Option<
        Vec<(
            crate::value_access::paths::ProjectionRoot,
            nocter_model::ValuePath<()>,
        )>,
    > {
        let mut pending = vec![(source, nocter_model::ValuePath::default(), BTreeSet::new())];
        let mut result = BTreeSet::new();
        while let Some((source, suffix, mut seen)) = pending.pop() {
            let mut projections = source.projections(
                |id| typed.body().expressions().get(id),
                |id| typed.body().blocks().get(id),
                crate::value_access::paths::ProjectionRoots::All,
                |_| (),
                |_, _| Some(()),
            )?;
            while let Some((root, path)) = projections.pop() {
                let path = path.concat(&suffix);
                let crate::value_access::paths::ProjectionRoot::Place(place) = root else {
                    result.insert((root, path));
                    continue;
                };
                let place = typed.body().places().get(place)?;
                let local = place.local;
                let path =
                    crate::value_access::paths::known_by(&place.fields, |_| ()).concat(&path);
                if let Some(&next) = self.sources.get(&local) {
                    let mut seen = if projections.is_empty() {
                        std::mem::take(&mut seen)
                    } else {
                        seen.clone()
                    };
                    if !seen.insert(local) {
                        return None;
                    }
                    pending.push((next, path, seen));
                } else {
                    result.insert((root, path));
                }
            }
        }
        Some(result.into_iter().collect())
    }
    fn resolve(
        &self,
        typed: &TypedQuery,
        mut source: Referent,
        whole: bool,
    ) -> Option<(AccessPlaceId, bool)> {
        let mut visited = BTreeSet::new();
        let mut aliased = false;
        loop {
            let place = if whole {
                let Referent::Place(place) = source else {
                    return None;
                };
                typed
                    .body()
                    .places()
                    .get(place)?
                    .fields
                    .is_root()
                    .then_some(place)?
            } else {
                source.static_base(
                    |id| typed.body().expressions().get(id),
                    |id| typed.body().blocks().get(id),
                )?
            };
            let local = typed.body().places().get(place)?.local;
            if let Some(&next) = self.sources.get(&local) {
                if !visited.insert(local) {
                    return None;
                }
                source = next;
                aliased = true;
            } else {
                return Some((place, aliased));
            }
        }
    }
    /// Exact whole-local identity only. Fields and evaluated selectors require other evidence.
    pub(super) fn whole_local(
        &self,
        typed: &TypedQuery,
        source: Referent,
    ) -> Option<AccessPlaceId> {
        self.resolve(typed, source, true).map(|(place, _)| place)
    }
    /// Containing root for may-write classification; never a definite replacement or address.
    pub(super) fn root(&self, typed: &TypedQuery, source: Referent) -> Option<AccessPlaceId> {
        self.resolve(typed, source, false).map(|(place, _)| place)
    }
    pub(super) fn footprint(&self, typed: &TypedQuery, source: Referent) -> Option<Referent> {
        let (place, aliased) = self.resolve(typed, source, false)?;
        // Frozen selectors must not be re-evaluated with the user's newer index.
        Some(if aliased {
            Referent::Place(place)
        } else {
            source
        })
    }
}
