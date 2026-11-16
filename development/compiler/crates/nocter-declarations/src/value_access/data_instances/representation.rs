//! Exact logical value representations across source input descriptors.
use super::{DataInstanceKey, InstanceSchema, Program};
use crate::value_access::AggregateForm;
use nocter_model::ValueField;
#[derive(Clone, Copy, Eq, PartialEq)]
enum Relation {
    Exact,
    Guarded,
    Value,
}

impl DataInstanceKey {
    /// Compare concrete logical representations without erasing code/environment or nominal
    /// identity. This grants no source conversion, ownership, physical geometry or initialized
    /// backing. Structural input constraints can name the same represented value differently.
    #[must_use]
    pub fn has_same_representation(&self, program: &Program, other: &Self) -> bool {
        self.compare_representation(program, other, Relation::Exact)
    }
    /// Check directed structural transport, permitting an excluded outcome alternative to enter
    /// a wider represented carrier. This is neither byte-copy compatibility nor an executed
    /// conversion: consumers must preserve tags, move active payloads and retire the source once.
    /// Empty roots, missing contexts and changed positive code/environment identities reject.
    #[must_use]
    pub fn permits_guarded_transport(&self, program: &Program, target: &Self) -> bool {
        !self.is_uninhabited() && self.compare_representation(program, target, Relation::Guarded)
    }
    /// Check structural value-domain injection, including complete positive callable choices.
    /// Existing positives must occur unchanged in the target; guarded exclusions may widen.
    /// This supplies no selector, initialized value, ownership, geometry or executed conversion.
    #[must_use]
    pub fn permits_value_transport(&self, program: &Program, target: &Self) -> bool {
        !self.is_uninhabited() && self.compare_representation(program, target, Relation::Value)
    }
    #[allow(
        clippy::too_many_lines,
        reason = "Keep comparison modes on one exhaustive schema edge policy."
    )]
    fn compare_representation(&self, program: &Program, other: &Self, relation: Relation) -> bool {
        let mut pending = vec![(self.clone(), other.clone(), false)];
        let mut seen = std::collections::BTreeSet::new();
        while let Some((left, right, guarded)) = pending.pop() {
            if !seen.insert((
                left.data,
                left.context_identity(),
                right.data,
                right.context_identity(),
                guarded,
            )) {
                continue;
            }
            // Equal closed descriptors need one complete validity check, not fresh construction
            // of every remaining suffix at each component of a deep or shared schema.
            if left.data == right.data && left.is_context_free() && right.is_context_free() {
                if program.instantiate_closed_data(left.data).is_err() {
                    return false;
                }
                continue;
            }
            let (Ok(a), Ok(b)) = (left.schema(program), right.schema(program)) else {
                return false;
            };
            match (a, b) {
                (InstanceSchema::Uninhabited, _)
                    if relation != Relation::Exact
                        && guarded
                        && program.matches_uninhabited_metadata_with(
                            left.data,
                            right.data,
                            |_| true,
                        ) => {}
                (InstanceSchema::Uninhabited, InstanceSchema::Uninhabited)
                    if relation == Relation::Exact
                        && program.matches_uninhabited_metadata_with(
                            left.data,
                            right.data,
                            |_| true,
                        ) => {}
                (InstanceSchema::Builtin(a), InstanceSchema::Builtin(b)) if a == b => {}
                (InstanceSchema::Function(a), InstanceSchema::Function(b)) if a == b => {}
                (
                    InstanceSchema::FunctionAlternatives(a),
                    InstanceSchema::FunctionAlternatives(b),
                ) if a == b => {}
                (
                    InstanceSchema::Function(actual),
                    InstanceSchema::FunctionAlternatives(target),
                ) if relation == Relation::Value && target.binary_search(&actual).is_ok() => {}
                (
                    InstanceSchema::FunctionAlternatives(actual),
                    InstanceSchema::FunctionAlternatives(target),
                ) if relation == Relation::Value
                    && actual.iter().all(|case| target.binary_search(case).is_ok()) => {}
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
                        pending.push((a, b, false));
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
                        pending.push((a, b, false));
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
                        pending.push((a, b, true));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}
