//! Exact logical value representations across source input descriptors.
use super::{DataInstanceKey, InstanceSchema, Program};
use crate::value_access::AggregateForm;
use nocter_model::ValueField;
use std::sync::Arc;

impl DataInstanceKey {
    /// Compare concrete logical representations without erasing code/environment or nominal
    /// identity. This grants no source conversion, ownership, physical geometry or initialized
    /// backing. Structural input constraints can name the same represented value differently.
    #[must_use]
    pub fn has_same_representation(&self, program: &Program, other: &Self) -> bool {
        let mut pending = vec![(self.clone(), other.clone())];
        let mut seen = std::collections::BTreeSet::new();
        while let Some((left, right)) = pending.pop() {
            if !seen.insert((
                left.data,
                Arc::as_ptr(&left.context),
                right.data,
                Arc::as_ptr(&right.context),
            )) {
                continue;
            }
            let (Ok(a), Ok(b)) = (left.schema(program), right.schema(program)) else {
                return false;
            };
            match (a, b) {
                (InstanceSchema::Builtin(a), InstanceSchema::Builtin(b)) if a == b => {}
                (InstanceSchema::Function(a), InstanceSchema::Function(b)) if a == b => {}
                (InstanceSchema::Aggregate(a), InstanceSchema::Aggregate(b)) => {
                    if a.len() != b.len() {
                        return false;
                    }
                    match (a.form(), b.form()) {
                        (AggregateForm::Tuple(_), AggregateForm::Tuple(_)) => {}
                        (AggregateForm::Record(a), AggregateForm::Record(b))
                            if a.data() == b.data() => {}
                        _ => return false,
                    }
                    for (a, b) in a.components().zip(b.components()) {
                        if a.field != b.field {
                            return false;
                        }
                        let (Ok(a), Ok(b)) = (
                            left.component(program, a.field),
                            right.component(program, b.field),
                        ) else {
                            return false;
                        };
                        pending.push((a, b));
                    }
                }
                (InstanceSchema::Array(a), InstanceSchema::Array(b))
                    if a.length() == b.length() =>
                {
                    // Empty metadata contains no positive element or selected callable.
                    if !a.is_empty() {
                        let (Ok(a), Ok(b)) = (
                            left.component(program, ValueField::Array(0)),
                            right.component(program, ValueField::Array(0)),
                        ) else {
                            return false;
                        };
                        pending.push((a, b));
                    }
                }
                (InstanceSchema::Optional(_), InstanceSchema::Optional(_))
                | (InstanceSchema::Fallible(_), InstanceSchema::Fallible(_)) => {
                    let Ok(fields) = program.instance_fields(left.data) else {
                        return false;
                    };
                    for (field, _) in fields {
                        let (Ok(a), Ok(b)) = (
                            left.component(program, field),
                            right.component(program, field),
                        ) else {
                            return false;
                        };
                        pending.push((a, b));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}
