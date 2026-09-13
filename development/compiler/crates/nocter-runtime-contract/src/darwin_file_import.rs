use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// One loader dependency admitted by the generated Darwin blocking-file service.
///
/// This is a closed allowlist. Generated file code receives typed identities and cannot select a
/// library or spell a loader symbol itself.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DarwinFileServiceFunction {
    Abort,
    Malloc,
    Free,
    DispatchQueueCreate,
    DispatchAsyncFunction,
    DispatchRelease,
    DispatchGroupCreate,
    DispatchGroupEnter,
    DispatchGroupLeave,
    DispatchGroupWait,
}

impl DarwinFileServiceFunction {
    pub const ALL: &'static [Self] = &[
        Self::Abort,
        Self::Malloc,
        Self::Free,
        Self::DispatchQueueCreate,
        Self::DispatchAsyncFunction,
        Self::DispatchRelease,
        Self::DispatchGroupCreate,
        Self::DispatchGroupEnter,
        Self::DispatchGroupLeave,
        Self::DispatchGroupWait,
    ];

    #[must_use]
    pub fn import(self) -> RuntimeFunctionImport {
        let symbol = match self {
            Self::Abort => "_abort",
            Self::Malloc => "_malloc",
            Self::Free => "_free",
            Self::DispatchQueueCreate => "_dispatch_queue_create",
            Self::DispatchAsyncFunction => "_dispatch_async_f",
            Self::DispatchRelease => "_dispatch_release",
            Self::DispatchGroupCreate => "_dispatch_group_create",
            Self::DispatchGroupEnter => "_dispatch_group_enter",
            Self::DispatchGroupLeave => "_dispatch_group_leave",
            Self::DispatchGroupWait => "_dispatch_group_wait",
        };
        RuntimeFunctionImport::trusted(RuntimeLibraryIdentity::DarwinSystem, symbol)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::DarwinFileServiceFunction;

    #[test]
    fn file_service_imports_are_complete_unique_system_functions() {
        let imports = DarwinFileServiceFunction::ALL
            .iter()
            .copied()
            .map(DarwinFileServiceFunction::import)
            .collect::<BTreeSet<_>>();
        assert_eq!(imports.len(), DarwinFileServiceFunction::ALL.len());
        assert!(
            imports
                .iter()
                .all(|import| import.library() == crate::RuntimeLibraryIdentity::DarwinSystem)
        );
    }
}
