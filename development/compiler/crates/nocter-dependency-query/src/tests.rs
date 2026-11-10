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
    assert_eq!(query.active_depth(), 0);

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

#[test]
fn consuming_a_query_freezes_only_completed_values() {
    let mut computation = graph(&[(0, &[1])]);
    let mut query = DependencyQuery::default();
    query.resolve(&mut computation, 0).unwrap();

    let completed = query.into_completed();

    assert_eq!(completed.get(&0).map(AsRef::as_ref), Some(&1));
    assert_eq!(completed.get(&1).map(AsRef::as_ref), Some(&1));
}

#[test]
fn active_keys_share_the_exact_query_path_and_disappear_on_completion() {
    struct Paths(Vec<Vec<u8>>);
    impl DependencyComputation<u8, (), ()> for Paths {
        fn compute(
            &mut self,
            query: &mut DependencyQuery<u8, ()>,
            key: u8,
        ) -> Result<(), DependencyQueryError<u8, ()>> {
            self.0.push(query.active_keys().copied().collect());
            if key > 0 {
                query.resolve(self, key - 1)?;
            }
            Ok(())
        }
    }
    let mut paths = Paths(Vec::new());
    let mut query = DependencyQuery::default();
    query.resolve(&mut paths, 2).unwrap();
    assert_eq!(paths.0, [vec![2], vec![2, 1], vec![2, 1, 0]]);
    assert_eq!(query.active_keys().len(), 0);
    assert_eq!(query.complete_len(), 3);
}

use super::{DependencyProgress, DependencyStep};

struct Steps {
    edges: HashMap<usize, Vec<usize>>,
    advances: HashMap<usize, usize>,
    completions: HashMap<usize, usize>,
    reject: Option<usize>,
    stall: Option<usize>,
}
impl Steps {
    fn new(edges: impl IntoIterator<Item = (usize, Vec<usize>)>) -> Self {
        Self {
            edges: edges.into_iter().collect(),
            advances: HashMap::new(),
            completions: HashMap::new(),
            reject: None,
            stall: None,
        }
    }
}
impl DependencyProgress<usize, usize, usize> for Steps {
    fn advance(
        &mut self,
        query: &DependencyQuery<usize, usize>,
        key: usize,
    ) -> Result<DependencyStep<usize, usize>, usize> {
        *self.advances.entry(key).or_default() += 1;
        if self.reject == Some(key) {
            return Err(key);
        }
        if self.stall == Some(key) {
            return Ok(DependencyStep::Require(Box::new([])));
        }
        let children = self.edges.get(&key).map_or(&[][..], Vec::as_slice);
        if children
            .iter()
            .any(|child| query.completed(child).is_none())
        {
            return Ok(DependencyStep::Require(children.into()));
        }
        *self.completions.entry(key).or_default() += 1;
        Ok(DependencyStep::Complete(
            1 + children
                .iter()
                .map(|child| query.completed(child).unwrap())
                .sum::<usize>(),
        ))
    }
    fn no_progress(&mut self, key: usize) -> usize {
        key
    }
}

#[test]
fn iterative_shared_groups_preserve_order_and_complete_each_key_once() {
    let mut steps = Steps::new([(0, vec![1, 2, 1]), (1, vec![3]), (2, vec![3])]);
    let mut query = DependencyQuery::default();
    assert_eq!(*query.resolve_steps(&mut steps, 0).unwrap(), 7);
    assert_eq!(steps.completions.len(), 4);
    assert!(steps.completions.values().all(|&count| count == 1));
    assert!(steps.advances.values().all(|&count| count <= 2));
    let same = query.resolve_steps(&mut steps, 0).unwrap();
    assert!(Arc::ptr_eq(
        &same,
        &query.resolve_steps(&mut steps, 0).unwrap()
    ));
    assert_eq!(query.active_depth(), 0);
}

#[test]
fn iterative_cycles_keep_completed_siblings_and_unwind_only_unfinished_work() {
    let mut steps = Steps::new([(0, vec![3, 1]), (1, vec![2]), (2, vec![1])]);
    let mut query = DependencyQuery::default();
    assert_eq!(
        query.resolve_steps(&mut steps, 0),
        Err(DependencyQueryError::Cycle(Box::new([1, 2, 1])))
    );
    assert_eq!(query.completed(&3), Some(&1));
    assert_eq!(query.complete_len(), 1);
    assert_eq!(query.active_depth(), 0);
    steps.edges.insert(2, vec![]);
    assert!(query.resolve_steps(&mut steps, 0).is_ok());
    assert_eq!(steps.completions[&3], 1);
}

#[test]
fn iterative_failures_and_stalled_drafts_are_not_published_and_can_be_retried() {
    for stalled in [false, true] {
        let mut steps = Steps::new([(0, vec![1, 2])]);
        if stalled {
            steps.stall = Some(2);
        } else {
            steps.reject = Some(2);
        }
        let mut query = DependencyQuery::default();
        assert_eq!(
            query.resolve_steps(&mut steps, 0),
            Err(DependencyQueryError::Computation(2))
        );
        assert_eq!(query.complete_len(), 1);
        assert_eq!(query.completed(&1), Some(&1));
        assert_eq!(query.active_depth(), 0);
        steps.stall = None;
        steps.reject = None;
        assert_eq!(*query.resolve_steps(&mut steps, 0).unwrap(), 3);
    }
}

#[test]
fn iterative_depth_does_not_use_the_host_call_stack() {
    const DEPTH: usize = 32768;
    let mut steps = Steps::new((0..DEPTH).map(|key| (key, vec![key + 1])));
    let mut query = DependencyQuery::default();
    assert_eq!(*query.resolve_steps(&mut steps, 0).unwrap(), DEPTH + 1);
    assert_eq!(query.complete_len(), DEPTH + 1);
    assert_eq!(query.active_depth(), 0);
}

#[test]
fn iterative_requests_of_only_completed_keys_report_no_progress() {
    struct Repeating;
    impl DependencyProgress<usize, usize, usize> for Repeating {
        fn advance(
            &mut self,
            _: &DependencyQuery<usize, usize>,
            key: usize,
        ) -> Result<DependencyStep<usize, usize>, usize> {
            if key == 0 {
                Ok(DependencyStep::Require(Box::new([1])))
            } else {
                Ok(DependencyStep::Complete(42))
            }
        }
        fn no_progress(&mut self, key: usize) -> usize {
            key
        }
    }
    let mut query = DependencyQuery::default();
    assert_eq!(
        query.resolve_steps(&mut Repeating, 0),
        Err(DependencyQueryError::Computation(0))
    );
    assert_eq!(query.completed(&1), Some(&42));
    assert_eq!(query.completed(&0), None);
    assert_eq!(query.active_depth(), 0);
}

#[test]
fn iterative_requests_share_outer_recursive_frames_and_preserve_them_on_failure() {
    struct Outer(Steps);
    impl DependencyComputation<usize, usize, usize> for Outer {
        fn compute(
            &mut self,
            query: &mut DependencyQuery<usize, usize>,
            key: usize,
        ) -> Result<usize, DependencyQueryError<usize, usize>> {
            assert_eq!(query.active_keys().copied().collect::<Vec<_>>(), [key]);
            assert_eq!(
                query.resolve_steps(&mut self.0, 1),
                Err(DependencyQueryError::Cycle(Box::new([0, 1, 0])))
            );
            assert_eq!(query.active_keys().copied().collect::<Vec<_>>(), [key]);
            assert!(query.completed(&key).is_none());
            self.0.edges.insert(1, vec![2]);
            Ok(*query.resolve_steps(&mut self.0, 1)? + 1)
        }
    }
    let mut outer = Outer(Steps::new([(1, vec![0])]));
    let mut query = DependencyQuery::default();
    assert_eq!(*query.resolve(&mut outer, 0).unwrap(), 3);
    assert_eq!(query.complete_len(), 3);
    assert_eq!(query.active_depth(), 0);
}
