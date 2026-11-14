//! Call-boundary equivalence excludes nonexistent empty-array elements, preserving positive data.
use super::Context;
use nocter_runtime_contract::{RuntimeDataId, RuntimeType, RuntimeTypeRepresentation};
use std::collections::BTreeSet;

impl Context<'_> {
    /// Calls can transport structural values whose empty-array element descriptors differ.
    /// Empty metadata supplies no element payload. All positive leaves, including static code
    /// tokens, retain their exact runtime identities; physical geometry is not this proof.
    pub(super) fn input_data_compatible(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> bool {
        let mut pending = vec![(actual, expected)];
        let mut seen = BTreeSet::new();
        let types = self.program.runtime.types();
        while let Some((actual, expected)) = pending.pop() {
            if !seen.insert((actual, expected)) {
                continue;
            }
            let (Some(a), Some(b)) = (types.get(actual), types.get(expected)) else {
                return false;
            };
            if actual == expected {
                continue;
            }
            match (a, b) {
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
                ) if al == bl => pending.push((*a, *b)),
                (RuntimeType::Optional(a), RuntimeType::Optional(b))
                | (RuntimeType::Fallible(a), RuntimeType::Fallible(b)) => pending.push((*a, *b)),
                (RuntimeType::Aggregate, RuntimeType::Aggregate) => {
                    let (
                        Some(RuntimeTypeRepresentation::ValueAggregate { fields: a }),
                        Some(RuntimeTypeRepresentation::ValueAggregate { fields: b }),
                    ) = (
                        self.program.runtime.type_representations().get(actual),
                        self.program.runtime.type_representations().get(expected),
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
                        pending.push((a.ty(), b.ty()));
                    }
                }
                _ => return false,
            }
        }
        true
    }
}
