//! Exact value representation and directed guarded transport share one runtime graph comparison.
use super::RuntimeEnvironment;
use crate::{RuntimeDataId, RuntimeType, RuntimeTypeRepresentation};
use std::collections::BTreeSet;

impl RuntimeEnvironment {
    /// Operations can transport structural values whose empty-array element descriptors differ.
    /// Empty metadata supplies no element payload. All positive leaves, including static code
    /// tokens, retain their exact runtime identities; physical geometry is not this proof.
    #[must_use]
    pub fn has_same_value_representation(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        self.compare_value_representation(actual, expected, false)
    }
    /// Check directed physical transport through excluded outcome alternatives. Positive runtime
    /// identities remain exact. This grants no source conversion, code/context proof, byte-copy
    /// compatibility, initialized value, layout or ownership transition.
    #[must_use]
    pub fn permits_guarded_value_transport(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        !matches!(
            self.types().get(actual),
            None | Some(RuntimeType::Uninhabited)
        ) && self.compare_value_representation(actual, expected, true)
    }
    fn compare_value_representation(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
        transport: bool,
    ) -> bool {
        let mut pending = vec![(actual, expected, false)];
        let mut seen = BTreeSet::new();
        let types = self.types();
        while let Some((actual, expected, guarded)) = pending.pop() {
            if !seen.insert((actual, expected, guarded)) {
                continue;
            }
            let (Some(a), Some(b)) = (types.get(actual), types.get(expected)) else {
                return false;
            };
            if actual == expected {
                continue;
            }
            match (a, b) {
                (RuntimeType::Uninhabited, _) if transport && guarded => {}
                (RuntimeType::EmptyArray { .. }, RuntimeType::EmptyArray { .. }) => {}
                (
                    RuntimeType::FixedArray {
                        element: a,
                        length: al,
                    },
                    RuntimeType::FixedArray {
                        element: b,
                        length: bl,
                    },
                ) if al == bl => pending.push((*a, *b, false)),
                (RuntimeType::Optional(a), RuntimeType::Optional(b))
                | (RuntimeType::Fallible(a), RuntimeType::Fallible(b)) => {
                    pending.push((*a, *b, true));
                }
                (RuntimeType::Aggregate, RuntimeType::Aggregate) => {
                    let (
                        Some(RuntimeTypeRepresentation::ValueAggregate { fields: a }),
                        Some(RuntimeTypeRepresentation::ValueAggregate { fields: b }),
                    ) = (
                        self.type_representations().get(actual),
                        self.type_representations().get(expected),
                    )
                    else {
                        return false;
                    };
                    if a.len() < 2 || a.len() != b.len() {
                        return false;
                    }
                    for (a, b) in a.iter().zip(b.iter()) {
                        if a.field() != b.field()
                            || !matches!(a.field(), nocter_model::ValueField::Tuple(_))
                        {
                            return false;
                        }
                        pending.push((a.ty(), b.ty(), false));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
