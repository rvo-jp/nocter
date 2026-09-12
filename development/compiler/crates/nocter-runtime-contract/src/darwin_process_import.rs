use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// One function dependency admitted by the closed Darwin process-cleanup service.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinProcessServiceFunction {
    Abort,
    Malloc,
    Free,
    DispatchAsyncFunction,
    DispatchGetGlobalQueue,
}

impl DarwinProcessServiceFunction {
    pub const ALL: &'static [Self] = &[
        Self::Abort,
        Self::Malloc,
        Self::Free,
        Self::DispatchAsyncFunction,
        Self::DispatchGetGlobalQueue,
    ];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        let symbol = match self {
            Self::Abort => "_abort",
            Self::Malloc => "_malloc",
            Self::Free => "_free",
            Self::DispatchAsyncFunction => "_dispatch_async_f",
            Self::DispatchGetGlobalQueue => "_dispatch_get_global_queue",
        };
        RuntimeFunctionImport::trusted(RuntimeLibraryIdentity::DarwinSystem, symbol)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DarwinProcessServiceFunction;

    #[test]
    fn process_service_imports_are_complete_and_unique() {
        let imports = DarwinProcessServiceFunction::ALL
            .iter()
            .copied()
            .map(DarwinProcessServiceFunction::import)
            .collect::<BTreeSet<_>>();
        assert_eq!(imports.len(), DarwinProcessServiceFunction::ALL.len());
    }
}
