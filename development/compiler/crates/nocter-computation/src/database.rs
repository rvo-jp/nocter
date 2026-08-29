use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::{ComputationError, ComputationIdentity, Fingerprint, Input, Query, QueryValue};

type Erased = dyn Any + Send + Sync;
type Evaluator = fn(&Database, &ComputationIdentity, Arc<Erased>) -> Result<(), ComputationError>;

/// Monotonic input revision owned by one computation database.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ComputationRevision(u64);

impl ComputationRevision {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone)]
struct Dependency {
    identity: ComputationIdentity,
    fingerprint: Fingerprint,
}

struct EvaluationFrame {
    identity: ComputationIdentity,
    dependencies: Vec<Dependency>,
}

enum EntryKind {
    Input,
    Query {
        key: Arc<Erased>,
        evaluator: Evaluator,
        dependencies: Vec<Dependency>,
    },
}

struct Entry {
    value: Arc<Erased>,
    fingerprint: Fingerprint,
    changed_at: ComputationRevision,
    verified_at: ComputationRevision,
    kind: EntryKind,
}

struct PendingInput {
    value: Arc<Erased>,
    fingerprint: Fingerprint,
}

#[derive(Default)]
struct Statistics {
    executions: BTreeMap<&'static str, u64>,
    reuses: BTreeMap<&'static str, u64>,
}

struct DatabaseState {
    revision: ComputationRevision,
    entries: BTreeMap<ComputationIdentity, Entry>,
    evaluations: Vec<EvaluationFrame>,
    statistics: Statistics,
}

impl Default for DatabaseState {
    fn default() -> Self {
        Self {
            revision: ComputationRevision(0),
            entries: BTreeMap::new(),
            evaluations: Vec::new(),
            statistics: Statistics::default(),
        }
    }
}

/// Sequential revisioned database for typed inputs and automatically tracked derived queries.
#[derive(Default)]
pub struct Database {
    state: RefCell<DatabaseState>,
}

impl fmt::Debug for Database {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("Database")
            .field("revision", &state.revision)
            .field("entries", &state.entries.len())
            .field("active_evaluations", &state.evaluations.len())
            .finish_non_exhaustive()
    }
}

impl Database {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn revision(&self) -> ComputationRevision {
        self.state.borrow().revision
    }

    /// Starts an atomic input transaction for the next revision.
    ///
    /// # Errors
    ///
    /// Returns [`ComputationError::RevisionExhausted`] when no later revision can be represented.
    pub fn advance_revision(&mut self) -> Result<InputRevision<'_>, ComputationError> {
        let current = self.state.get_mut().revision;
        let next = current
            .0
            .checked_add(1)
            .ok_or(ComputationError::RevisionExhausted)?;
        Ok(InputRevision {
            database: self,
            revision: ComputationRevision(next),
            pending: BTreeMap::new(),
        })
    }

    /// Reads one exact input and records it as a dependency of the active query.
    ///
    /// # Errors
    ///
    /// Returns a missing-input or internal stored-type failure.
    pub fn input<I: Input>(&self, key: &I::Key) -> Result<Arc<I::Value>, ComputationError> {
        let identity = ComputationIdentity::input::<I>(key);
        let fingerprint = self.ensure(&identity)?;
        let value = self.value::<I::Value>(&identity)?;
        self.record_dependency(identity, fingerprint);
        Ok(value)
    }

    /// Evaluates or reuses one typed derived query and records it in its active parent.
    ///
    /// # Errors
    ///
    /// Returns a missing input, dependency cycle, provider failure, or internal stored-type failure.
    pub fn query<Q: Query>(&self, key: Q::Key) -> Result<Arc<Q::Value>, ComputationError> {
        let identity = ComputationIdentity::query::<Q>(&key);
        if self.state.borrow().entries.contains_key(&identity) {
            self.ensure(&identity)?;
        } else {
            self.evaluate::<Q>(key)?;
        }
        let fingerprint = self
            .state
            .borrow()
            .entries
            .get(&identity)
            .map(|entry| entry.fingerprint)
            .ok_or_else(|| ComputationError::StoredTypeMismatch(identity.clone()))?;
        let value = self.value::<Q::Value>(&identity)?;
        self.record_dependency(identity, fingerprint);
        Ok(value)
    }

    #[must_use]
    pub fn execution_count<Q: Query>(&self) -> u64 {
        self.state
            .borrow()
            .statistics
            .executions
            .get(std::any::type_name::<Q>())
            .copied()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn reuse_count<Q: Query>(&self) -> u64 {
        self.state
            .borrow()
            .statistics
            .reuses
            .get(std::any::type_name::<Q>())
            .copied()
            .unwrap_or(0)
    }

    fn value<T: Send + Sync + 'static>(
        &self,
        identity: &ComputationIdentity,
    ) -> Result<Arc<T>, ComputationError> {
        let value = self
            .state
            .borrow()
            .entries
            .get(identity)
            .map(|entry| Arc::clone(&entry.value))
            .ok_or_else(|| ComputationError::StoredTypeMismatch(identity.clone()))?;
        Arc::downcast(value).map_err(|_| ComputationError::StoredTypeMismatch(identity.clone()))
    }

    fn ensure(&self, identity: &ComputationIdentity) -> Result<Fingerprint, ComputationError> {
        enum Validation {
            Current(Fingerprint),
            Input,
            Query {
                key: Arc<Erased>,
                evaluator: Evaluator,
                dependencies: Vec<Dependency>,
            },
        }

        let validation = {
            let state = self.state.borrow();
            let Some(entry) = state.entries.get(identity) else {
                return Err(ComputationError::MissingInput(identity.clone()));
            };
            if entry.verified_at == state.revision {
                Validation::Current(entry.fingerprint)
            } else {
                match &entry.kind {
                    EntryKind::Input => Validation::Input,
                    EntryKind::Query {
                        key,
                        evaluator,
                        dependencies,
                    } => Validation::Query {
                        key: Arc::clone(key),
                        evaluator: *evaluator,
                        dependencies: dependencies.clone(),
                    },
                }
            }
        };

        match validation {
            Validation::Current(fingerprint) => Ok(fingerprint),
            Validation::Input => {
                let mut state = self.state.borrow_mut();
                let revision = state.revision;
                let entry = state
                    .entries
                    .get_mut(identity)
                    .ok_or_else(|| ComputationError::MissingInput(identity.clone()))?;
                entry.verified_at = revision;
                Ok(entry.fingerprint)
            }
            Validation::Query {
                key,
                evaluator,
                dependencies,
            } => {
                let mut dirty = false;
                for dependency in dependencies {
                    let current = self.ensure(&dependency.identity)?;
                    if current != dependency.fingerprint {
                        dirty = true;
                        break;
                    }
                }
                if dirty {
                    evaluator(self, identity, key)?;
                } else {
                    let mut state = self.state.borrow_mut();
                    let revision = state.revision;
                    state
                        .entries
                        .get_mut(identity)
                        .ok_or_else(|| ComputationError::StoredTypeMismatch(identity.clone()))?
                        .verified_at = revision;
                    *state.statistics.reuses.entry(identity.name()).or_default() += 1;
                }
                self.state
                    .borrow()
                    .entries
                    .get(identity)
                    .map(|entry| entry.fingerprint)
                    .ok_or_else(|| ComputationError::StoredTypeMismatch(identity.clone()))
            }
        }
    }

    fn evaluate<Q: Query>(&self, key: Q::Key) -> Result<(), ComputationError> {
        let identity = ComputationIdentity::query::<Q>(&key);
        let evaluation = self.begin_evaluation(&identity)?;
        let result = Q::execute(self, &key);
        let dependencies = evaluation.finish();
        let value = result?;
        let fingerprint = value.fingerprint();
        let value: Arc<Erased> = Arc::new(value);
        let key: Arc<Erased> = Arc::new(key);
        let mut state = self.state.borrow_mut();
        let revision = state.revision;
        let changed_at = state
            .entries
            .get(&identity)
            .filter(|entry| entry.fingerprint == fingerprint)
            .map_or(revision, |entry| entry.changed_at);
        state.entries.insert(
            identity.clone(),
            Entry {
                value,
                fingerprint,
                changed_at,
                verified_at: revision,
                kind: EntryKind::Query {
                    key,
                    evaluator: evaluate_erased::<Q>,
                    dependencies,
                },
            },
        );
        *state
            .statistics
            .executions
            .entry(identity.name())
            .or_default() += 1;
        Ok(())
    }

    fn begin_evaluation(
        &self,
        identity: &ComputationIdentity,
    ) -> Result<EvaluationGuard<'_>, ComputationError> {
        let mut state = self.state.borrow_mut();
        if let Some(start) = state
            .evaluations
            .iter()
            .position(|frame| &frame.identity == identity)
        {
            let mut cycle = state.evaluations[start..]
                .iter()
                .map(|frame| frame.identity.clone())
                .collect::<Vec<_>>();
            cycle.push(identity.clone());
            return Err(ComputationError::Cycle(cycle.into_boxed_slice()));
        }
        state.evaluations.push(EvaluationFrame {
            identity: identity.clone(),
            dependencies: Vec::new(),
        });
        Ok(EvaluationGuard {
            database: self,
            identity: identity.clone(),
            open: true,
        })
    }

    fn end_evaluation(&self, identity: &ComputationIdentity) -> Vec<Dependency> {
        let frame = self
            .state
            .borrow_mut()
            .evaluations
            .pop()
            .expect("every evaluation frame is closed");
        assert_eq!(
            &frame.identity, identity,
            "query evaluation stack is nested"
        );
        frame.dependencies
    }

    fn record_dependency(&self, identity: ComputationIdentity, fingerprint: Fingerprint) {
        let mut state = self.state.borrow_mut();
        let Some(frame) = state.evaluations.last_mut() else {
            return;
        };
        if frame
            .dependencies
            .iter()
            .any(|dependency| dependency.identity == identity)
        {
            return;
        }
        frame.dependencies.push(Dependency {
            identity,
            fingerprint,
        });
    }
}

struct EvaluationGuard<'database> {
    database: &'database Database,
    identity: ComputationIdentity,
    open: bool,
}

impl EvaluationGuard<'_> {
    fn finish(mut self) -> Vec<Dependency> {
        self.open = false;
        self.database.end_evaluation(&self.identity)
    }
}

impl Drop for EvaluationGuard<'_> {
    fn drop(&mut self) {
        if self.open {
            self.database.end_evaluation(&self.identity);
        }
    }
}

/// Exclusive input mutation for one newly advanced database revision.
pub struct InputRevision<'database> {
    database: &'database mut Database,
    revision: ComputationRevision,
    pending: BTreeMap<ComputationIdentity, PendingInput>,
}

impl InputRevision<'_> {
    #[must_use]
    pub const fn revision(&self) -> ComputationRevision {
        self.revision
    }

    pub fn set<I: Input>(&mut self, key: &I::Key, value: I::Value) {
        let identity = ComputationIdentity::input::<I>(key);
        let fingerprint = value.fingerprint();
        let value: Arc<Erased> = Arc::new(value);
        self.pending
            .insert(identity, PendingInput { value, fingerprint });
    }

    /// Atomically publishes every staged input and advances the visible database revision.
    #[must_use]
    pub fn commit(self) -> ComputationRevision {
        let state = self.database.state.get_mut();
        for (identity, pending) in self.pending {
            let changed_at = state
                .entries
                .get(&identity)
                .filter(|entry| entry.fingerprint == pending.fingerprint)
                .map_or(self.revision, |entry| entry.changed_at);
            state.entries.insert(
                identity,
                Entry {
                    value: pending.value,
                    fingerprint: pending.fingerprint,
                    changed_at,
                    verified_at: self.revision,
                    kind: EntryKind::Input,
                },
            );
        }
        state.revision = self.revision;
        self.revision
    }
}

fn evaluate_erased<Q: Query>(
    database: &Database,
    identity: &ComputationIdentity,
    key: Arc<Erased>,
) -> Result<(), ComputationError> {
    let key = Arc::downcast::<Q::Key>(key)
        .map_err(|_| ComputationError::StoredTypeMismatch(identity.clone()))?;
    database.evaluate::<Q>((*key).clone())
}
