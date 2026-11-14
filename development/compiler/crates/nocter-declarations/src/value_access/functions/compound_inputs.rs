//! Match compound input constraints without erasing the supplying value's static witnesses.
use super::Program;
use crate::value_access::{AggregateForm, PayloadSchema};
use nocter_model::{TypeId, ValueFunctionOrigin, ValueParameter};
use std::collections::BTreeSet;

impl Program {
    /// Match ordinary structural data and callable component contracts at an input boundary.
    /// This does not retype a value or grant ownership, invocation or representation evidence.
    /// Nested invocation adapters require a separate value-producing operation and are not
    /// manufactured by metadata matching.
    #[must_use]
    pub fn matches_compound_input(&self, actual: TypeId, expected: TypeId) -> bool {
        let mut pending = vec![(actual, expected)];
        let mut seen = BTreeSet::new();
        while let Some((actual, expected)) = pending.pop() {
            if !seen.insert((actual, expected)) {
                continue;
            }
            let (Some(supplied), Some(required)) =
                (self.data_schema(actual), self.data_schema(expected))
            else {
                return false;
            };
            if actual == expected {
                continue;
            }
            match (supplied, required) {
                (PayloadSchema::Function(supplied), PayloadSchema::Function(required))
                    if matches!(
                        required.origin,
                        ValueFunctionOrigin::Parameter(ValueParameter::Component { .. })
                    ) =>
                {
                    let (Some(supplied), Some(required)) = (
                        self.function_contract(supplied),
                        self.function_contract(required),
                    ) else {
                        return false;
                    };
                    if supplied != required {
                        return false;
                    }
                }
                (PayloadSchema::Aggregate(supplied), PayloadSchema::Aggregate(required)) => {
                    let (AggregateForm::Tuple(supplied), AggregateForm::Tuple(required)) =
                        (supplied.form(), required.form())
                    else {
                        return false;
                    };
                    if supplied.len() != required.len() {
                        return false;
                    }
                    pending.extend(supplied.iter().copied().zip(required.iter().copied()));
                }
                (PayloadSchema::Array(supplied), PayloadSchema::Array(required)) => {
                    if supplied.length() != required.length() {
                        return false;
                    }
                    pending.push((supplied.element(), required.element()));
                }
                (PayloadSchema::Optional(supplied), PayloadSchema::Optional(required))
                | (PayloadSchema::Fallible(supplied), PayloadSchema::Fallible(required)) => {
                    pending.push((supplied, required));
                }
                _ => return false,
            }
        }
        true
    }
}
