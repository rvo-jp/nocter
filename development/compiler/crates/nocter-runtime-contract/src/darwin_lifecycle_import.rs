use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// Function dependencies admitted by process-owned Darwin lifecycle observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinLifecycleFunction {
    Signal,
}

impl DarwinLifecycleFunction {
    pub const ALL: &'static [Self] = &[Self::Signal];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        match self {
            Self::Signal => {
                RuntimeFunctionImport::trusted(RuntimeLibraryIdentity::DarwinSystem, "_signal")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DarwinLifecycleFunction;

    #[test]
    fn lifecycle_imports_are_complete_and_unique() {
        let imports = DarwinLifecycleFunction::ALL
            .iter()
            .copied()
            .map(DarwinLifecycleFunction::import)
            .collect::<BTreeSet<_>>();
        assert_eq!(imports.len(), DarwinLifecycleFunction::ALL.len());
    }
}
