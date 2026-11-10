//! Construction-scoped dependency state, immutable memoization and explicit request progress.
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
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

/// The next construction action: completion, authored-order dependencies or bounded computation.
#[derive(Debug)]
pub enum DependencyStep<K, V> {
    Complete(V),
    Require(Box<[K]>),
    /// Run a bounded re-entrant computation for this already-active key.
    Compute,
}

/// Advances unpublished work through explicit dependency groups or bounded re-entrant computation.
pub trait DependencyProgress<K, V, E> {
    /// A required group must contain an unfinished key. Completed dependencies may occur in the
    /// group and are skipped. A draft completes through `Complete` or the result of `Compute`.
    /// # Errors
    /// Returns the consumer's authored or integrity failure; only the query constructs cycles.
    fn advance(&mut self, query: &DependencyQuery<K, V>, key: K)
    -> Result<DependencyStep<K, V>, E>;
    /// Compute the current active key without reopening it. Nested requests use this same query.
    /// Re-resolving the current key would correctly report a cycle; compute it directly instead.
    /// The consumer must bound recursive work; only `Require` groups use explicit query frames.
    /// # Errors
    /// Propagates exact query cycles or consumer failures. The default rejects this unsupported role.
    fn compute_current(
        &mut self,
        _: &mut DependencyQuery<K, V>,
        key: K,
    ) -> Result<V, DependencyQueryError<K, E>> {
        Err(DependencyQueryError::Computation(self.no_progress(key)))
    }
    /// Classify a draft that requests no unfinished dependency and therefore cannot progress.
    fn no_progress(&mut self, key: K) -> E;
}

#[derive(Debug)]
struct Active<K> {
    key: K,
    pending: std::vec::IntoIter<K>,
    dependencies: RefCell<BTreeSet<usize>>,
}

#[derive(Debug)]
enum QueryState {
    Active,
    Complete(usize),
}

/// One completed fact and its direct successful prerequisites, in completion order.
/// Every prerequisite index is strictly smaller than this node's index.
#[derive(Debug)]
pub struct CompletedDependency<K, V> {
    pub key: K,
    pub value: Arc<V>,
    pub dependencies: Box<[usize]>,
}

/// One memoized dependency authority for a semantic construction.
///
/// A key is either absent, active on the exact current stack, or complete. Failed computations
/// remove their active state before returning, so no partial result can be observed or reused.
#[derive(Debug)]
pub struct DependencyQuery<K, V> {
    states: HashMap<K, QueryState>,
    stack: Vec<Active<K>>,
    completed: Vec<CompletedDependency<K, V>>,
}

impl<K, V> Default for DependencyQuery<K, V> {
    fn default() -> Self {
        Self {
            states: HashMap::new(),
            stack: Vec::new(),
            completed: Vec::new(),
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
        if let Some(value) = self.begin(key.clone())? {
            return Ok(value);
        }
        let result = computation.compute(self, key.clone());
        self.finish(key, result)
    }

    /// Resolve a dependency closure with query-owned frames instead of host recursion.
    /// Shares all states and the active path with `resolve`, including when called from it.
    /// # Errors
    /// Returns an exact cycle, an authored failure or the consumer's no-progress failure. Unwinds
    /// only this invocation's unfinished frames; completed values and outer active work survive.
    /// # Panics
    /// Panics only if this query's private frame/state invariants lose integrity.
    pub fn resolve_steps<E>(
        &mut self,
        computation: &mut impl DependencyProgress<K, V, E>,
        key: K,
    ) -> Result<Arc<V>, DependencyQueryError<K, E>> {
        let outer_depth = self.stack.len();
        if let Some(value) = self.begin(key)? {
            return Ok(value);
        }
        loop {
            let frame = self.stack.last_mut().expect("active step frame");
            if let Some(child) = frame.pending.next() {
                if let Err(error) = self.begin(child) {
                    self.unwind(outer_depth);
                    return Err(error);
                }
                continue;
            }
            let key = frame.key.clone();
            let result = match computation.advance(self, key.clone()) {
                Ok(DependencyStep::Complete(value)) => Ok(value),
                Ok(DependencyStep::Compute) => computation.compute_current(self, key.clone()),
                Ok(DependencyStep::Require(dependencies)) => {
                    if !dependencies.iter().any(|key| self.completed(key).is_none()) {
                        let error = DependencyQueryError::Computation(computation.no_progress(key));
                        self.unwind(outer_depth);
                        return Err(error);
                    }
                    self.stack.last_mut().expect("current step frame").pending =
                        dependencies.into_vec().into_iter();
                    continue;
                }
                Err(error) => Err(DependencyQueryError::Computation(error)),
            };
            match self.finish(key, result) {
                Ok(value) if self.stack.len() == outer_depth => return Ok(value),
                Ok(_) => {}
                Err(error) => {
                    self.unwind(outer_depth);
                    return Err(error);
                }
            }
        }
    }

    fn begin<E>(&mut self, key: K) -> Result<Option<Arc<V>>, DependencyQueryError<K, E>> {
        match self.states.get(&key) {
            Some(QueryState::Complete(index)) => {
                self.depend_on(*index);
                return Ok(Some(Arc::clone(&self.completed[*index].value)));
            }
            Some(QueryState::Active) => {
                let start = self
                    .stack
                    .iter()
                    .position(|frame| frame.key == key)
                    .expect("active key must occur on the query path");
                let mut cycle: Vec<_> = self.stack[start..]
                    .iter()
                    .map(|frame| frame.key.clone())
                    .collect();
                cycle.push(key);
                return Err(DependencyQueryError::Cycle(cycle.into_boxed_slice()));
            }
            None => {}
        }
        self.states.insert(key.clone(), QueryState::Active);
        self.stack.push(Active {
            key,
            pending: Vec::new().into_iter(),
            dependencies: RefCell::new(BTreeSet::new()),
        });
        Ok(None)
    }
    fn finish<E>(
        &mut self,
        key: K,
        result: Result<V, DependencyQueryError<K, E>>,
    ) -> Result<Arc<V>, DependencyQueryError<K, E>> {
        let popped = self.stack.pop().expect("retain current dependency frame");
        assert!(
            popped.key == key,
            "dependency computation changed query stack order"
        );
        match result {
            Ok(value) => {
                let value = Arc::new(value);
                let index = self.completed.len();
                self.states.insert(key.clone(), QueryState::Complete(index));
                self.completed.push(CompletedDependency {
                    key,
                    value: Arc::clone(&value),
                    dependencies: popped.dependencies.into_inner().into_iter().collect(),
                });
                self.depend_on(index);
                Ok(value)
            }
            Err(error) => {
                self.states.remove(&key);
                Err(error)
            }
        }
    }
    fn unwind(&mut self, outer_depth: usize) {
        while self.stack.len() > outer_depth {
            let frame = self.stack.pop().expect("unfinished dependency frame");
            self.states.remove(&frame.key);
        }
    }

    fn depend_on(&self, index: usize) {
        if let Some(frame) = self.stack.last() {
            frame.dependencies.borrow_mut().insert(index);
        }
    }

    /// Read a completed value, recording it as a prerequisite of the current active computation.
    #[must_use]
    pub fn completed(&self, key: &K) -> Option<&V> {
        match self.states.get(key) {
            Some(QueryState::Complete(index)) => {
                self.depend_on(*index);
                Some(self.completed[*index].value.as_ref())
            }
            Some(QueryState::Active) | None => None,
        }
    }

    #[must_use]
    pub fn complete_len(&self) -> usize {
        self.completed.len()
    }

    /// Number of pending dependency computations, including the current computation.
    /// Consumers use this same stack authority when sharing a depth budget with execution.
    #[must_use]
    pub fn active_depth(&self) -> usize {
        self.stack.len()
    }
    /// Read the exact pending request path, from outermost to current. Role-specific execution
    /// budgets can count their own keys without maintaining a second dependency stack.
    #[must_use]
    pub fn active_keys(&self) -> impl DoubleEndedIterator<Item = &K> + ExactSizeIterator {
        self.stack.iter().map(|frame| &frame.key)
    }

    /// Consumes the authority and returns exactly its completed immutable values.
    ///
    /// Active states cannot survive an externally observable resolution call, and failed branches
    /// are removed before it returns. Consuming the query therefore freezes one complete result
    /// set without cloning either the query state or its values.
    #[must_use]
    pub fn into_completed(self) -> HashMap<K, Arc<V>> {
        self.completed
            .into_iter()
            .map(|node| (node.key, node.value))
            .collect()
    }

    /// Freeze successful facts and tracked prerequisites without cloning the query or values.
    /// Failed and still-active computations have no nodes. Indices form a flat DAG, so deep
    /// dependency chains do not cause recursive traversal or destruction.
    #[must_use]
    pub fn into_dependencies(self) -> Vec<CompletedDependency<K, V>> {
        self.completed
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
mod tests;
