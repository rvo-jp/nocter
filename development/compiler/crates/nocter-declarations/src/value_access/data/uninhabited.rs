//! Excluded descriptors retain contracts without inventing positive callable identities.
use super::{PayloadSchema, Program};
use nocter_model::{TypeId, ValueFunctionOrigin};
use std::collections::BTreeSet;

impl Program {
    /// Compare exact data, permitting equivalent callable constraints only below empty arrays.
    /// All inhabited leaves and nominal identities remain exact. This provides no element,
    /// concrete code, source conversion, ownership or layout permission.
    #[must_use]
    pub fn matches_uninhabited_metadata(&self, actual: TypeId, expected: TypeId) -> bool {
        self.matches_uninhabited_metadata_with(actual, expected, |_| false)
    }
    /// Extend metadata comparison with an independently established empty-domain proof for both
    /// callable leaves. Contracts and nominal identities remain exact; no value or layout is granted.
    #[must_use]
    pub fn matches_uninhabited_metadata_with(
        &self,
        actual: TypeId,
        expected: TypeId,
        uninhabited: impl Fn(crate::value_access::FunctionWitness) -> bool,
    ) -> bool {
        self.matches_empty_constraints(actual, expected, uninhabited, false)
    }
    /// Compare directed symbolic transport contracts. Only an independently excluded source
    /// callable beneath an outcome can enter a different compatible target contract. This
    /// grants no concrete context, initialized value, byte-copy or executable conversion.
    #[must_use]
    pub fn matches_guarded_transport_metadata(
        &self,
        actual: TypeId,
        expected: TypeId,
        uninhabited: impl Fn(crate::value_access::FunctionWitness) -> bool,
    ) -> bool {
        self.matches_empty_constraints(actual, expected, uninhabited, true)
    }
    fn matches_empty_constraints(
        &self,
        actual: TypeId,
        expected: TypeId,
        uninhabited: impl Fn(crate::value_access::FunctionWitness) -> bool,
        transport: bool,
    ) -> bool {
        let mut pending = vec![(actual, expected, false, false)];
        let mut seen = BTreeSet::new();
        while let Some((actual, expected, metadata, guarded)) = pending.pop() {
            let (Some(a), Some(b)) = (self.data_schema(actual), self.data_schema(expected)) else {
                return false;
            };
            if actual == expected {
                continue;
            }
            if !seen.insert((actual, expected, metadata, guarded)) {
                continue;
            }
            match (a, b) {
                (PayloadSchema::Function(a), PayloadSchema::Function(b))
                    if ((metadata && (descriptor(a.origin) || descriptor(b.origin)))
                        || (uninhabited(a)
                            && ((transport && guarded) || (!transport && uninhabited(b)))))
                        && self
                            .function_contract(a)
                            .zip(self.function_contract(b))
                            .is_some_and(|(a, b)| a == b) => {}
                (PayloadSchema::Array(a), PayloadSchema::Array(b)) if a.length() == b.length() => {
                    pending.push((a.element(), b.element(), metadata || a.is_empty(), guarded));
                }
                (PayloadSchema::Aggregate(a), PayloadSchema::Aggregate(b)) => {
                    use crate::value_access::AggregateForm;
                    let (AggregateForm::Tuple(a), AggregateForm::Tuple(b)) = (a.form(), b.form())
                    else {
                        return false;
                    };
                    if a.len() != b.len() {
                        return false;
                    }
                    pending.extend(
                        a.iter()
                            .copied()
                            .zip(b.iter().copied())
                            .map(|(a, b)| (a, b, metadata, guarded)),
                    );
                }
                (PayloadSchema::Optional(a), PayloadSchema::Optional(b))
                | (PayloadSchema::Fallible(a), PayloadSchema::Fallible(b)) => {
                    pending.push((a, b, metadata, true));
                }
                _ => return false,
            }
        }
        true
    }
}
fn descriptor(origin: ValueFunctionOrigin) -> bool {
    matches!(
        origin,
        ValueFunctionOrigin::Parameter(_)
            | ValueFunctionOrigin::Result(_)
            | ValueFunctionOrigin::Application(_)
    )
}
