//! Absence is a source fact, independent of candidate-set cardinality and join order.
use super::{InputProducers, Producers};

#[test]
fn described_unknown_cancels_exclusion_but_unobserved_accumulators_do_not() {
    let absent = InputProducers::source(Producers::new(), true);
    let unknown = InputProducers::from(Producers::new());
    assert!(absent.excludes_active_callable());
    assert!(!unknown.excludes_active_callable());
    let mut observed = InputProducers::default();
    observed.join(absent.clone());
    assert_eq!(observed, absent);
    observed.join(InputProducers::default());
    assert_eq!(observed, absent);
    observed.join(absent.clone());
    assert_eq!(observed, absent);
    observed.join(unknown.clone());
    assert!(!observed.excludes_active_callable());
    let mut reverse = unknown;
    reverse.join(absent);
    assert_eq!(observed, reverse);
    assert!(observed.producers().is_empty());
}
