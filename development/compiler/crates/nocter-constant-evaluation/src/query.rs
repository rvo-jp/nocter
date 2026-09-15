use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

/// Computes one dependency value while retaining the same query authority for recursive requests.
pub trait DependencyComputation<K, V, E> {
    /// Computes `key` from dependency values requested through `query`.
    ///
    /// Implementations must report authored or integrity failures as `Computation`; active-query
    /// cycles are produced only by the query authority itself.
    ///
    /// # Errors
    ///
    /// Returns a typed computation failure or propagates a dependency cycle/failure from a
    /// recursive query request.
    fn compute(
        &mut self,
        query: &mut DependencyQuery<K, V>,
        key: K,
    ) -> Result<V, DependencyQueryError<K, E>>;
}

#[derive(Debug)]
enum QueryState<V> {
    Active,
    Complete(Arc<V>),
}

/// One memoized dependency authority for a semantic construction.
///
/// A key is either absent, active on the exact current stack, or complete. Failed computations
/// remove their active state before returning, so no partial result can be observed or reused.
#[derive(Debug)]
pub struct DependencyQuery<K, V> {
    states: HashMap<K, QueryState<V>>,
    stack: Vec<K>,
}

impl<K, V> Default for DependencyQuery<K, V> {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            stack: Vec::new(),
        }
    }
}

impl<K, V> DependencyQuery<K, V>
where
    K: Clone + Eq + Hash,
{
    /// Resolves one key, reusing a completed value and rejecting active re-entry.
    ///
    /// # Errors
    ///
    /// Returns the exact active cycle or the computation's typed failure. Neither failure retains
    /// a partial value or active stack entry.
    ///
    /// # Panics
    ///
    /// Panics only if this type's private active-state and stack representations disagree. The
    /// computation contract has no access to either representation and cannot cause that state.
    pub fn resolve<E>(
        &mut self,
        computation: &mut impl DependencyComputation<K, V, E>,
        key: K,
    ) -> Result<Arc<V>, DependencyQueryError<K, E>> {
        match self.states.get(&key) {
            Some(QueryState::Complete(value)) => return Ok(Arc::clone(value)),
            Some(QueryState::Active) => {
                let start = self
                    .stack
                    .iter()
                    .position(|active| active == &key)
                    .expect("active dependency key must occur on the query stack");
                let mut cycle = self.stack[start..].to_vec();
                cycle.push(key);
                return Err(DependencyQueryError::Cycle(cycle.into_boxed_slice()));
            }
            None => {}
        }

        self.states.insert(key.clone(), QueryState::Active);
        self.stack.push(key.clone());
        let result = computation.compute(self, key.clone());
        let popped = self
            .stack
            .pop()
            .expect("dependency computation must retain its query stack entry");
        assert!(
            popped == key,
            "dependency computation changed query stack order"
        );
        match result {
            Ok(value) => {
                let value = Arc::new(value);
                self.states
                    .insert(key, QueryState::Complete(Arc::clone(&value)));
                Ok(value)
            }
            Err(error) => {
                self.states.remove(&key);
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn completed(&self, key: &K) -> Option<&V> {
        match self.states.get(key) {
            Some(QueryState::Complete(value)) => Some(value.as_ref()),
            Some(QueryState::Active) | None => None,
        }
    }

    #[must_use]
    pub fn complete_len(&self) -> usize {
        self.states
            .values()
            .filter(|state| matches!(state, QueryState::Complete(_)))
            .count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyQueryError<K, E> {
    Cycle(Box<[K]>),
    Computation(E),
}

impl<K, E> DependencyQueryError<K, E> {
    #[must_use]
    pub const fn computation(error: E) -> Self {
        Self::Computation(error)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use super::{DependencyComputation, DependencyQuery, DependencyQueryError};

    struct Graph {
        edges: HashMap<u8, Vec<u8>>,
        visits: HashMap<u8, usize>,
        reject: Option<u8>,
    }

    impl DependencyComputation<u8, u64, u8> for Graph {
        fn compute(
            &mut self,
            query: &mut DependencyQuery<u8, u64>,
            key: u8,
        ) -> Result<u64, DependencyQueryError<u8, u8>> {
            *self.visits.entry(key).or_default() += 1;
            if self.reject == Some(key) {
                return Err(DependencyQueryError::computation(key));
            }
            let dependencies = self.edges.get(&key).cloned().unwrap_or_default();
            let mut value = u64::from(key);
            for dependency in dependencies {
                value += *query.resolve(self, dependency)?;
            }
            Ok(value)
        }
    }

    fn graph(edges: &[(u8, &[u8])]) -> Graph {
        Graph {
            edges: edges
                .iter()
                .map(|(key, dependencies)| (*key, dependencies.to_vec()))
                .collect(),
            visits: HashMap::new(),
            reject: None,
        }
    }

    #[test]
    fn diamond_dependency_is_computed_once() {
        let mut computation = graph(&[(0, &[1, 2]), (1, &[3]), (2, &[3])]);
        let mut query = DependencyQuery::default();

        assert_eq!(query.resolve(&mut computation, 0).as_deref(), Ok(&9));
        assert_eq!(computation.visits.get(&3), Some(&1));
        assert_eq!(query.complete_len(), 4);
    }

    #[test]
    fn cycle_contains_the_exact_active_path_and_publishes_nothing() {
        let mut computation = graph(&[(0, &[1]), (1, &[2]), (2, &[1])]);
        let mut query = DependencyQuery::default();

        assert_eq!(
            query.resolve(&mut computation, 0),
            Err(DependencyQueryError::Cycle(Box::new([1, 2, 1])))
        );
        assert_eq!(query.complete_len(), 0);
        assert!(query.completed(&0).is_none());
    }

    #[test]
    fn failed_branch_can_be_recomputed_after_its_cause_changes() {
        let mut computation = graph(&[(0, &[1])]);
        computation.reject = Some(1);
        let mut query = DependencyQuery::default();

        assert_eq!(
            query.resolve(&mut computation, 0),
            Err(DependencyQueryError::Computation(1))
        );
        assert_eq!(query.complete_len(), 0);

        computation.reject = None;
        assert_eq!(query.resolve(&mut computation, 0).as_deref(), Ok(&1));
    }

    #[test]
    fn repeated_resolution_shares_one_completed_value() {
        let mut computation = graph(&[]);
        let mut query = DependencyQuery::default();

        let first = query.resolve(&mut computation, 7).unwrap();
        let second = query.resolve(&mut computation, 7).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(computation.visits.get(&7), Some(&1));
    }
}
