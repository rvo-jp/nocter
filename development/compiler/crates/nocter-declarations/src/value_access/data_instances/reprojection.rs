//! Retain each supplied occurrence while selecting an equivalent declaration descriptor.
use super::{DataInstanceKey, InstanceSchema, Program};
use crate::value_access::{AggregateForm, CodeBindingError, PayloadSchema};
use nocter_model::{TypeId, ValueField};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const INVALID: CodeBindingError = CodeBindingError::InvalidWitness;
type Node = (TypeId, usize, TypeId);

fn node(source: &DataInstanceKey, target: TypeId) -> Node {
    (
        source.data(),
        if source.is_context_free() {
            0
        } else {
            Arc::as_ptr(&source.context) as usize
        },
        target,
    )
}

impl DataInstanceKey {
    /// Select another existing compatible descriptor without replacing any occurrence's code,
    /// environment or represented domain. This grants no source conversion, initialized value,
    /// physical transport or reuse receipt. Empty domains remain empty rather than acquiring code.
    /// # Errors
    /// Rejects missing contexts, changed positive witnesses, incompatible schemas and cycles.
    pub fn reproject(&self, program: &Program, target: TypeId) -> Result<Self, CodeBindingError> {
        let root = node(self, target);
        let mut pending = vec![(self.clone(), target, false)];
        let mut known = BTreeMap::new();
        let mut active = BTreeSet::new();
        while let Some((source, target, expanded)) = pending.pop() {
            let selected = node(&source, target);
            if known.contains_key(&selected) {
                continue;
            }
            let schema = source.schema(program)?;
            let expected = program.data_schema(target).ok_or(INVALID)?;
            let ready = if source.data() == target {
                Some(source.clone())
            } else {
                match (&schema, expected) {
                    (InstanceSchema::Uninhabited, _)
                        if program.matches_uninhabited_metadata_with(
                            source.data(),
                            target,
                            |_| true,
                        ) =>
                    {
                        Some(program.instantiate_uninhabited_data(target)?)
                    }
                    (InstanceSchema::Builtin(a), PayloadSchema::Builtin(b)) if *a == b => {
                        Some(program.instantiate_closed_data(target)?)
                    }
                    (InstanceSchema::Function(actual), PayloadSchema::Function(_)) => {
                        let value = program.instantiate_function(target, actual)?;
                        if !matches!(value.schema(program)?, InstanceSchema::Function(selected) if selected == *actual)
                        {
                            return Err(INVALID);
                        }
                        Some(value)
                    }
                    _ => None,
                }
            };
            if let Some(value) = ready {
                known.insert(selected, value);
                continue;
            }
            let children = components(program, &source, target)?;
            if expanded {
                let values = children
                    .into_iter()
                    .map(|(field, source, target)| {
                        known
                            .get(&node(&source, target))
                            .cloned()
                            .map(|key| (field, key))
                            .ok_or(INVALID)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let value = program.instantiate_components(target, values)?;
                // Every child preserves its domain and witness. The matched parent schema and
                // canonical constructor preserve the whole representation without another walk.
                known.insert(selected, value);
                active.remove(&selected);
            } else {
                if !active.insert(selected) {
                    return Err(INVALID);
                }
                pending.push((source, target, true));
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|(_, source, target)| (source, target, false)),
                );
            }
        }
        known.remove(&root).ok_or(INVALID)
    }
}

fn components(
    program: &Program,
    source: &DataInstanceKey,
    target: TypeId,
) -> Result<Vec<(ValueField, DataInstanceKey, TypeId)>, CodeBindingError> {
    match (
        source.schema(program)?,
        program.data_schema(target).ok_or(INVALID)?,
    ) {
        (InstanceSchema::Aggregate(a), PayloadSchema::Aggregate(b)) if a.len() == b.len() => {
            match (a.form(), b.form()) {
                (AggregateForm::Tuple(_), AggregateForm::Tuple(_)) => {}
                (AggregateForm::Record(a), AggregateForm::Record(b)) if a.data() == b.data() => {}
                _ => return Err(INVALID),
            }
        }
        (InstanceSchema::Array(a), PayloadSchema::Array(b)) if a.length() == b.length() => {}
        (InstanceSchema::Optional(_), PayloadSchema::Optional(_))
        | (InstanceSchema::Fallible(_), PayloadSchema::Fallible(_)) => {}
        _ => return Err(INVALID),
    }
    program
        .instance_fields(target)?
        .into_iter()
        .map(|(field, data)| Ok((field, source.component(program, field)?, data)))
        .collect()
}
