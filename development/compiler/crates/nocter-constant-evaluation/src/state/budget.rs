//! Deterministic work quotas and live evaluator storage, independent of target/host layout.
use super::Rule;
use crate::CompileTimeEvaluationLimits;
use std::{
    num::{NonZeroU32, NonZeroU64},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// Stateful execution adds a live-storage bound to the shared work/depth limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    work: CompileTimeEvaluationLimits,
    storage: NonZeroU64,
}
impl Limits {
    #[must_use]
    pub const fn new(work: CompileTimeEvaluationLimits, storage: NonZeroU64) -> Self {
        Self { work, storage }
    }
    #[must_use]
    pub const fn steps(self) -> NonZeroU64 {
        self.work.steps()
    }
    #[must_use]
    pub const fn call_depth(self) -> NonZeroU32 {
        self.work.call_depth()
    }
    #[must_use]
    pub const fn storage(self) -> NonZeroU64 {
        self.storage
    }
}
impl From<CompileTimeEvaluationLimits> for Limits {
    fn from(work: CompileTimeEvaluationLimits) -> Self {
        Self::new(
            work,
            NonZeroU64::new(1_048_576).expect("nonzero storage limit"),
        )
    }
}
impl Default for Limits {
    fn default() -> Self {
        CompileTimeEvaluationLimits::default().into()
    }
}
/// A deterministic work quota, independent of evaluator frames, call depth and live values.
/// Separate compilation activities may share the policy without spending execution operations.
pub struct WorkBudget {
    limit: NonZeroU64,
    remaining: u64,
}
impl WorkBudget {
    #[must_use]
    pub const fn new(limit: NonZeroU64) -> Self {
        Self {
            limit,
            remaining: limit.get(),
        }
    }
    pub fn restart(&mut self) {
        self.remaining = self.limit.get();
    }
    /// # Errors
    /// Reports exhausted deterministic work. A failed charge exhausts the quota.
    pub fn charge(&mut self, units: u64) -> Result<(), Rule> {
        let Some(remaining) = self.remaining.checked_sub(units) else {
            self.remaining = 0;
            return Err(Rule::StepLimit);
        };
        self.remaining = remaining;
        Ok(())
    }
    /// # Errors
    /// Reports exhausted work without replenishing the quota.
    pub fn step(&mut self) -> Result<(), Rule> {
        self.charge(1)
    }
}

/// Work resets only at an outer request; storage remains shared while any result is retained.
pub struct Budget {
    limits: Limits,
    work: WorkBudget,
    pub(super) storage: Storage,
}
impl Budget {
    #[must_use]
    pub fn new(limits: impl Into<Limits>) -> Self {
        let limits = limits.into();
        Self {
            limits,
            work: WorkBudget::new(limits.steps()),
            storage: Storage(Arc::new(Meter {
                limit: limits.storage().get(),
                used: AtomicU64::new(0),
            })),
        }
    }
    #[must_use]
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    /// Start an outer request without forgetting storage held by completed dependencies.
    pub fn restart_work(&mut self) {
        self.work.restart();
    }
    /// # Errors
    /// Reports exhausted deterministic work without replenishing a nested request's budget.
    pub fn step(&mut self) -> Result<(), Rule> {
        self.work.step()
    }
    #[must_use]
    pub fn storage_used(&self) -> u64 {
        self.storage.0.used.load(Ordering::Relaxed)
    }
}
struct Meter {
    limit: u64,
    used: AtomicU64,
}
#[derive(Clone)]
pub(super) struct Storage(Arc<Meter>);
/// One physical allocation/entry owns one charge. Sharing its owner never charges twice.
pub(super) struct Lease {
    meter: Arc<Meter>,
    units: u64,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let previous = self.meter.used.fetch_sub(self.units, Ordering::Relaxed);
        debug_assert!(previous >= self.units, "storage lease released twice");
    }
}
impl Lease {
    pub(super) fn belongs_to(&self, storage: &Storage) -> bool {
        Arc::ptr_eq(&self.meter, &storage.0)
    }
    pub(super) fn grow(&mut self, storage: &Storage, units: usize) -> Result<(), Rule> {
        let units = u64::try_from(units).map_err(|_| Rule::StorageLimit)?;
        if !self.belongs_to(storage) {
            return Err(Rule::InvalidPlan);
        }
        let total = self.units.checked_add(units).ok_or(Rule::StorageLimit)?;
        let mut additional = storage.reserve(units)?;
        self.units = total;
        additional.units = 0;
        Ok(())
    }
}
/// Entry metadata is distinct from any shared values or paths stored in it.
pub(super) struct Resident<T> {
    pub(super) value: T,
    _lease: Lease,
}
impl<T> std::ops::Deref for Resident<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.value
    }
}
impl<T> std::ops::DerefMut for Resident<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
impl Storage {
    pub(super) fn reserve(&self, units: u64) -> Result<Lease, Rule> {
        self.0
            .used
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(units)
                    .filter(|&total| total <= self.0.limit)
            })
            .map_err(|_| Rule::StorageLimit)?;
        Ok(Lease {
            meter: self.0.clone(),
            units,
        })
    }
    pub(super) fn items(&self, count: usize) -> Result<Lease, Rule> {
        let units = u64::try_from(count)
            .ok()
            .and_then(|n| n.checked_add(1))
            .ok_or(Rule::StorageLimit)?;
        self.reserve(units)
    }
    pub(super) fn entry<T>(&self, value: T) -> Result<Resident<T>, Rule> {
        Ok(Resident {
            value,
            _lease: self.reserve(2)?,
        })
    }
}
