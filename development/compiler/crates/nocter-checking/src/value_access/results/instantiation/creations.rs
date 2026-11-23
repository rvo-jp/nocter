//! Creation-frame substitution through component snapshots, never outer-name guessing.
use super::{Fact, InstantiationFailure, Key, Query};
use nocter_declarations::value_access::{
    AggregateForm, CodeInstanceKey, FunctionWitness, PayloadSchema, Program,
};
use nocter_model::ValueFunctionOrigin as Origin;
use nocter_model::{FieldPath, TypeId, ValueClosure, ValueField};
use std::collections::BTreeMap;

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
        let root = (
            recipe.environment,
            schema.environment(),
            recipe.producers.identity(),
        );
        let mut pending = vec![(
            recipe.environment,
            schema.environment(),
            recipe.producers.clone(),
            false,
        )];
        let mut completed = BTreeMap::new();
        while let Some((source, target, producers, expanded)) = pending.pop() {
            let node = (source, target, producers.identity());
            if completed.contains_key(&node) {
                continue;
            }
            if !expanded {
                self.step()?;
                if let (Some(PayloadSchema::Function(_)), Some(PayloadSchema::Function(_))) = (
                    self.program.data_schema(source),
                    self.program.data_schema(target),
                ) {
                    let value = match self.union(
                        owner,
                        frame,
                        producers.producers_at(&FieldPath::default()),
                    )? {
                        Fact::Witness(actual) => {
                            match self.program.instantiate_function(target, &actual) {
                                Ok(data) => Some(data),
                                Err(_) => return Ok(Fact::Unavailable),
                            }
                        }
                        Fact::Empty => None,
                        _ => return Ok(Fact::Unavailable),
                    };
                    completed.insert(node, value);
                    continue;
                }
            }
            let Some(children) = children(self.program, source, target) else {
                return Ok(Fact::Unavailable);
            };
            if !expanded {
                pending.push((source, target, producers.clone(), true));
                for (field, source, target) in children {
                    self.step()?;
                    pending.push((source, target, producers.component(field), false));
                }
                continue;
            }
            let mut values = Vec::new();
            let mut unresolved = false;
            for (field, source, target) in children {
                match completed.get(&(source, target, producers.component(field).identity())) {
                    Some(Some(value)) => values.push((field, value.clone())),
                    Some(None) => unresolved = true,
                    None => return Ok(Fact::Unavailable),
                }
            }
            let value = if unresolved {
                None
            } else {
                let value = if matches!(
                    self.program.data_schema(target),
                    Some(PayloadSchema::Builtin(_) | PayloadSchema::Enumeration(_))
                ) {
                    self.program.instantiate_closed_data(target)
                } else {
                    self.program.instantiate_components(target, values)
                };
                match value {
                    Ok(value) => Some(value),
                    Err(_) => return Ok(Fact::Unavailable),
                }
            };
            completed.insert(node, value);
        }
        let Some(Some(environment)) = completed.remove(&root) else {
            return Ok(Fact::Empty);
        };
        Ok(self
            .program
            .bind_callable_data(selected, &environment)
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
            (PayloadSchema::Enumeration(a), PayloadSchema::Enumeration(b))
                if a.data() == b.data() =>
            {
                Vec::new()
            }
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
            (PayloadSchema::Optional(a), PayloadSchema::Optional(b)) => {
                vec![(ValueField::Variant(true), a, b)]
            }
            (PayloadSchema::Fallible(a), PayloadSchema::Fallible(b)) => {
                let error = program.data_type(nocter_model::BuiltinType::Error)?;
                vec![
                    (ValueField::Variant(true), a, b),
                    (ValueField::Variant(false), error, error),
                ]
            }
            _ => return None,
        },
    )
}
