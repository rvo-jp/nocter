use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// Function dependencies admitted by process-owned Darwin termination observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinTerminationFunction {
    Signal,
}

impl DarwinTerminationFunction {
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

    use super::DarwinTerminationFunction;

    #[test]
    fn termination_imports_are_complete_and_unique() {
        let imports = DarwinTerminationFunction::ALL
            .iter()
            .copied()
            .map(DarwinTerminationFunction::import)
            .collect::<BTreeSet<_>>();
        assert_eq!(imports.len(), DarwinTerminationFunction::ALL.len());
    }
}
