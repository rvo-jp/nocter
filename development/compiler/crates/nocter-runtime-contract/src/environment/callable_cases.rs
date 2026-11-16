//! Complete-case correspondence is metadata, not permission to move an active value.
use super::RuntimeEnvironment;
use crate::{RuntimeDataId, RuntimeType};
use std::collections::BTreeMap;

/// One original source case and its index in the destination's canonical domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableCaseTarget {
    data: RuntimeDataId,
    target: u64,
}
impl CallableCaseTarget {
    #[must_use]
    pub const fn data(self) -> RuntimeDataId {
        self.data
    }
    #[must_use]
    pub const fn target(self) -> u64 {
        self.target
    }
}

impl RuntimeEnvironment {
    /// Map every source case to an exactly identical complete destination case.
    /// Rows follow source-domain order; a static source has one row and no stored source tag.
    /// Rejects unknown/non-callable data, positive narrowing and missing complete identities.
    /// This supplies no active selector, ownership, conversion, initialization or executable proof.
    #[must_use]
    pub fn callable_case_targets(
        &self,
        actual: RuntimeDataId,
        expected: RuntimeDataId,
    ) -> Option<Box<[CallableCaseTarget]>> {
        let RuntimeType::CallableAlternatives(targets) = self.types().get(expected)? else {
            return None;
        };
        let targets = targets
            .iter()
            .enumerate()
            .map(|(index, data)| Some((*data, u64::try_from(index).ok()?)))
            .collect::<Option<BTreeMap<_, _>>>()?;
        let source = match self.types().get(actual)? {
            RuntimeType::StaticCallable { .. } => std::slice::from_ref(&actual),
            RuntimeType::CallableAlternatives(cases) => cases.as_ref(),
            _ => return None,
        };
        source
            .iter()
            .map(|data| {
                Some(CallableCaseTarget {
                    data: *data,
                    target: *targets.get(data)?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
