//! Creation-frame substitution through original structural snapshots, never outer-name guessing.
use super::{Fact, InstantiationFailure, Key, Query};
use nocter_declarations::value_access::{
    AggregateForm, CodeInstanceKey, ConcreteWitness, FunctionWitness, PayloadSchema, Program,
};
use nocter_model::ValueFunctionOrigin as Origin;
use nocter_model::{FieldPath, TypeId, ValueClosure, ValueField, ValueParameter};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Bindings(BTreeMap<ValueParameter, ConcreteWitness>);
impl Bindings {
    /// Simultaneous occurrences may supply different views of the same context. This is not
    /// an alternative join: the producer union at each occurrence must already be unique.
    fn supply(
        &mut self,
        program: &Program,
        parameter: ValueParameter,
        supplied: ConcreteWitness,
    ) -> Result<(), ()> {
        let Some(previous) = self.0.get(&parameter) else {
            self.0.insert(parameter, supplied);
            return Ok(());
        };
        if previous.invocation().can_supply(supplied.invocation()) {
            let previous = program
                .adapt_witness(previous, supplied.invocation())
                .map_err(|_| ())?;
            return (previous == supplied).then_some(()).ok_or(());
        }
        let weaker = program
            .adapt_witness(&supplied, previous.invocation())
            .map_err(|_| ())?;
        if &weaker != previous {
            return Err(());
        }
        self.0.insert(parameter, supplied);
        Ok(())
    }
    fn leaf(
        &mut self,
        program: &Program,
        wanted: FunctionWitness,
        actual: &ConcreteWitness,
    ) -> Result<(), ()> {
        let actual = program
            .adapt_witness(actual, wanted.invocation)
            .map_err(|_| ())?;
        if let Origin::Parameter(parameter) = wanted.origin {
            return self.supply(program, parameter, actual);
        }
        if actual.witness() != wanted {
            return Err(());
        }
        for (&parameter, witness) in actual.captures() {
            self.supply(program, parameter, witness.clone())?;
        }
        Ok(())
    }
}

impl Query<'_, '_> {
    pub(super) fn creation(
        &mut self,
        owner: &Key,
        frame: &CodeInstanceKey,
        closure: ValueClosure,
    ) -> Result<Fact, InstantiationFailure> {
        let Some(recipe) = self.evidence.creation(closure) else {
            return Ok(Fact::Unavailable);
        };
        if recipe.owner != frame.code() {
            return Ok(Fact::Unavailable);
        }
        let Some(schema) = self.program.closure(closure) else {
            return Ok(Fact::Unavailable);
        };
        let selected = FunctionWitness {
            origin: Origin::Closure(closure),
            invocation: schema.contract().invocation,
        };
        let mut pending = vec![(
            recipe.environment,
            schema.environment(),
            recipe.producers.clone(),
        )];
        let mut seen = BTreeSet::new();
        let mut bindings = Bindings::default();
        let mut unresolved = false;
        while let Some((source, target, producers)) = pending.pop() {
            if !seen.insert((source, target, producers.identity())) {
                continue;
            }
            self.step()?;
            if let (Some(PayloadSchema::Function(_)), Some(PayloadSchema::Function(wanted))) = (
                self.program.data_schema(source),
                self.program.data_schema(target),
            ) {
                match self.union(owner, frame, producers.producers_at(&FieldPath::default()))? {
                    Fact::Witness(actual) => {
                        if bindings.leaf(self.program, wanted, &actual).is_err() {
                            return Ok(Fact::Unavailable);
                        }
                    }
                    Fact::Empty => unresolved = true,
                    _ => return Ok(Fact::Unavailable),
                }
                continue;
            }
            let Some(children) = children(self.program, source, target) else {
                return Ok(Fact::Unavailable);
            };
            for (field, source, target) in children {
                self.step()?;
                pending.push((source, target, producers.component(field)));
            }
        }
        if unresolved {
            return Ok(Fact::Empty);
        }
        Ok(self
            .program
            .bind_callable_environment(selected, bindings.0)
            .map_or(Fact::Unavailable, Fact::Witness))
    }
}

fn children(
    program: &Program,
    source: TypeId,
    target: TypeId,
) -> Option<Vec<(ValueField, TypeId, TypeId)>> {
    Some(
        match (program.data_schema(source)?, program.data_schema(target)?) {
            (PayloadSchema::Builtin(a), PayloadSchema::Builtin(b)) if a == b => Vec::new(),
            (PayloadSchema::Aggregate(a), PayloadSchema::Aggregate(b))
                if a.len() == b.len()
                    && (source == target
                        || matches!(
                            (a.form(), b.form()),
                            (AggregateForm::Tuple(_), AggregateForm::Tuple(_))
                        )) =>
            {
                a.components()
                    .zip(b.components())
                    .map(|(a, b)| (b.field, a.data, b.data))
                    .collect()
            }
            (PayloadSchema::Array(a), PayloadSchema::Array(b)) if a.length() == b.length() => {
                if a.is_empty() {
                    Vec::new()
                } else {
                    vec![(ValueField::Array(0), a.element(), b.element())]
                }
            }
            (PayloadSchema::Optional(a), PayloadSchema::Optional(b))
            | (PayloadSchema::Fallible(a), PayloadSchema::Fallible(b)) => {
                vec![(ValueField::Variant(true), a, b)]
            }
            _ => return None,
        },
    )
}
