use crate::{ComputationError, Database, Fingerprint, Input, Query, QueryValue};

struct NumberInput;

impl Input for NumberInput {
    type Key = ();
    type Value = Number;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Number(i32);

impl QueryValue for Number {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes(&self.0.to_be_bytes())
    }
}

struct Parity;

impl Query for Parity {
    type Key = ();
    type Value = Number;

    fn execute(database: &Database, (): &()) -> Result<Number, ComputationError> {
        Ok(Number(database.input::<NumberInput>(&())?.0 & 1))
    }
}

struct Label;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Text(&'static str);

impl QueryValue for Text {
    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes(self.0.as_bytes())
    }
}

impl Query for Label {
    type Key = ();
    type Value = Text;

    fn execute(database: &Database, (): &()) -> Result<Text, ComputationError> {
        Ok(if database.query::<Parity>(())?.0 == 0 {
            Text("even")
        } else {
            Text("odd")
        })
    }
}

#[test]
fn unchanged_intermediate_fingerprint_stops_invalidation() {
    let mut database = Database::new();
    let mut revision = database.advance_revision().unwrap();
    revision.set::<NumberInput>(&(), Number(1));
    assert_eq!(revision.commit().get(), 1);
    assert_eq!(*database.query::<Label>(()).unwrap(), Text("odd"));
    assert_eq!(database.execution_count::<Parity>(), 1);
    assert_eq!(database.execution_count::<Label>(), 1);

    let mut revision = database.advance_revision().unwrap();
    revision.set::<NumberInput>(&(), Number(3));
    assert_eq!(revision.commit().get(), 2);
    assert_eq!(*database.query::<Label>(()).unwrap(), Text("odd"));
    assert_eq!(database.execution_count::<Parity>(), 2);
    assert_eq!(database.execution_count::<Label>(), 1);
    assert_eq!(database.reuse_count::<Label>(), 1);
}

#[test]
fn unchanged_inputs_reuse_the_complete_dependency_chain() {
    let mut database = Database::new();
    let mut revision = database.advance_revision().unwrap();
    revision.set::<NumberInput>(&(), Number(2));
    assert_eq!(revision.commit().get(), 1);
    assert_eq!(*database.query::<Label>(()).unwrap(), Text("even"));

    assert_eq!(database.advance_revision().unwrap().commit().get(), 2);
    assert_eq!(*database.query::<Label>(()).unwrap(), Text("even"));
    assert_eq!(database.execution_count::<Parity>(), 1);
    assert_eq!(database.execution_count::<Label>(), 1);
    assert_eq!(database.reuse_count::<Parity>(), 1);
    assert_eq!(database.reuse_count::<Label>(), 1);
}

struct Recursive;

impl Query for Recursive {
    type Key = u64;
    type Value = Number;

    fn execute(database: &Database, key: &u64) -> Result<Number, ComputationError> {
        database.query::<Self>(*key).map(|value| *value)
    }
}

#[test]
fn recursive_queries_report_the_exact_cycle() {
    let database = Database::new();
    let error = database.query::<Recursive>(7).unwrap_err();
    let ComputationError::Cycle(cycle) = error else {
        panic!("expected cycle");
    };
    assert_eq!(cycle.len(), 2);
    assert_eq!(cycle[0], cycle[1]);
    assert_eq!(cycle[0].key(), 7_u64.to_be_bytes());
}

#[test]
fn absent_inputs_are_not_fabricated() {
    let database = Database::new();
    assert!(matches!(
        database.query::<Parity>(()),
        Err(ComputationError::MissingInput(_))
    ));
}

struct Panicking;

impl Query for Panicking {
    type Key = ();
    type Value = Number;

    fn execute(_: &Database, (): &()) -> Result<Number, ComputationError> {
        panic!("provider panic")
    }
}

#[test]
fn provider_panics_do_not_poison_the_evaluation_stack() {
    let database = Database::new();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        database.query::<Panicking>(())
    }));
    assert!(panic.is_err());
    let retry = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        database.query::<Panicking>(())
    }));
    assert!(retry.is_err());
}

#[test]
fn abandoned_input_transactions_do_not_publish_partial_revisions() {
    let mut database = Database::new();
    let initial = database.revision();
    {
        let mut revision = database.advance_revision().unwrap();
        revision.set::<NumberInput>(&(), Number(4));
    }
    assert_eq!(database.revision(), initial);
    assert!(matches!(
        database.input::<NumberInput>(&()),
        Err(ComputationError::MissingInput(_))
    ));
}
