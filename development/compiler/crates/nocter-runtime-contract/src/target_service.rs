use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_model::{CallableId, CompilationTarget};

use crate::{RuntimeFunctionImport, RuntimeLibraryIdentity};

/// One value lane admitted by the private target-service calling convention.
///
/// This is intentionally smaller than Nocter's stored-value domain. Aggregate transport,
/// floating-point registers, callbacks, and foreign ownership cannot enter the initial service
/// boundary accidentally.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetServiceValueAbi {
    CInt,
    Word,
    Pointer,
}

/// Foreign calling convention fixed by a trusted target-service descriptor.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TargetServiceCallingConvention {
    PlatformC,
}

/// One complete fixed-arity foreign signature selected by the trusted target catalog.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetServiceSignature {
    parameters: Box<[TargetServiceValueAbi]>,
    result: Option<TargetServiceValueAbi>,
}

impl TargetServiceSignature {
    #[must_use]
    fn new(
        parameters: impl Into<Box<[TargetServiceValueAbi]>>,
        result: Option<TargetServiceValueAbi>,
    ) -> Self {
        Self {
            parameters: parameters.into(),
            result,
        }
    }

    #[must_use]
    pub const fn parameters(&self) -> &[TargetServiceValueAbi] {
        &self.parameters
    }

    #[must_use]
    pub const fn result(&self) -> Option<TargetServiceValueAbi> {
        self.result
    }
}

/// Compiler-owned identity for one callable in the finite target-service catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetServiceRole {
    DarwinGetAddressInfo,
    DarwinFreeAddressInfo,
}

impl TargetServiceRole {
    pub const ALL: &'static [Self] = &[Self::DarwinGetAddressInfo, Self::DarwinFreeAddressInfo];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DarwinGetAddressInfo => "darwin_get_address_info",
            Self::DarwinFreeAddressInfo => "darwin_free_address_info",
        }
    }

    #[must_use]
    fn descriptor(self) -> TargetServiceDescriptor {
        match self {
            Self::DarwinGetAddressInfo => TargetServiceDescriptor::new(
                CompilationTarget::Arm64Darwin,
                TargetServiceCallingConvention::PlatformC,
                RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_getaddrinfo")
                    .expect("catalog symbol is valid"),
                TargetServiceSignature::new(
                    [
                        TargetServiceValueAbi::Pointer,
                        TargetServiceValueAbi::Pointer,
                        TargetServiceValueAbi::Pointer,
                        TargetServiceValueAbi::Pointer,
                    ],
                    Some(TargetServiceValueAbi::CInt),
                ),
            ),
            Self::DarwinFreeAddressInfo => TargetServiceDescriptor::new(
                CompilationTarget::Arm64Darwin,
                TargetServiceCallingConvention::PlatformC,
                RuntimeFunctionImport::new(RuntimeLibraryIdentity::DarwinSystem, "_freeaddrinfo")
                    .expect("catalog symbol is valid"),
                TargetServiceSignature::new([TargetServiceValueAbi::Pointer], None),
            ),
        }
    }
}

/// Closed source-independent call plan for one trusted operating-system function.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TargetServiceDescriptor {
    target: CompilationTarget,
    calling_convention: TargetServiceCallingConvention,
    import: RuntimeFunctionImport,
    signature: TargetServiceSignature,
}

impl TargetServiceDescriptor {
    #[must_use]
    const fn new(
        target: CompilationTarget,
        calling_convention: TargetServiceCallingConvention,
        import: RuntimeFunctionImport,
        signature: TargetServiceSignature,
    ) -> Self {
        Self {
            target,
            calling_convention,
            import,
            signature,
        }
    }

    #[must_use]
    pub const fn target(&self) -> CompilationTarget {
        self.target
    }

    #[must_use]
    pub const fn calling_convention(&self) -> TargetServiceCallingConvention {
        self.calling_convention
    }

    #[must_use]
    pub const fn import(&self) -> &RuntimeFunctionImport {
        &self.import
    }

    #[must_use]
    pub const fn signature(&self) -> &TargetServiceSignature {
        &self.signature
    }
}

/// Exact semantic callable attached to one trusted target-service role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetServiceBinding {
    role: TargetServiceRole,
    callable: CallableId,
    descriptor: TargetServiceDescriptor,
}

impl TargetServiceBinding {
    #[must_use]
    pub fn new(role: TargetServiceRole, callable: CallableId) -> Self {
        Self {
            role,
            callable,
            descriptor: role.descriptor(),
        }
    }

    #[must_use]
    pub const fn role(&self) -> TargetServiceRole {
        self.role
    }

    #[must_use]
    pub const fn callable(&self) -> CallableId {
        self.callable
    }

    #[must_use]
    pub const fn descriptor(&self) -> &TargetServiceDescriptor {
        &self.descriptor
    }
}

/// Complete target-service attachment in canonical role order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetServiceRegistry {
    bindings: Box<[TargetServiceBinding]>,
}

impl TargetServiceRegistry {
    /// An explicitly unavailable target-service capability set.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            bindings: Box::new([]),
        }
    }

    /// Freezes either an explicitly unavailable capability set or the complete trusted catalog.
    ///
    /// # Errors
    ///
    /// Rejects missing or duplicate roles and callables attached to more than one service.
    pub fn new(
        bindings: impl IntoIterator<Item = TargetServiceBinding>,
    ) -> Result<Self, TargetServiceBindingError> {
        let mut by_role = BTreeMap::new();
        let mut callables = BTreeSet::new();
        for binding in bindings {
            let role = binding.role();
            let callable = binding.callable();
            if by_role.insert(role, binding).is_some() {
                return Err(TargetServiceBindingError::DuplicateRole(role));
            }
            if !callables.insert(callable) {
                return Err(TargetServiceBindingError::DuplicateCallable(callable));
            }
        }
        if by_role.is_empty() {
            return Ok(Self::empty());
        }
        let mut canonical = Vec::with_capacity(TargetServiceRole::ALL.len());
        for role in TargetServiceRole::ALL {
            canonical.push(
                by_role
                    .remove(role)
                    .ok_or(TargetServiceBindingError::MissingRole(*role))?,
            );
        }
        Ok(Self {
            bindings: canonical.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn bindings(&self) -> &[TargetServiceBinding] {
        &self.bindings
    }

    #[must_use]
    pub fn binding(&self, callable: CallableId) -> Option<&TargetServiceBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.callable() == callable)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetServiceBindingError {
    MissingRole(TargetServiceRole),
    DuplicateRole(TargetServiceRole),
    DuplicateCallable(CallableId),
}

impl fmt::Display for TargetServiceBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRole(_) => {
                formatter.write_str("target-service registry is missing a role")
            }
            Self::DuplicateRole(_) => {
                formatter.write_str("target-service registry contains a duplicate role")
            }
            Self::DuplicateCallable(_) => formatter
                .write_str("target-service registry attaches one callable to multiple roles"),
        }
    }
}

impl std::error::Error for TargetServiceBindingError {}

#[cfg(test)]
mod tests {
    use nocter_model::{ArenaBuilder, CallableId, CompilationTarget};

    use super::{
        TargetServiceBinding, TargetServiceBindingError, TargetServiceCallingConvention,
        TargetServiceRegistry, TargetServiceRole, TargetServiceValueAbi,
    };
    use crate::RuntimeLibraryIdentity;

    #[test]
    fn target_service_catalog_is_complete_and_canonical() {
        let mut callables = ArenaBuilder::<CallableId, ()>::new();
        let mut bindings = TargetServiceRole::ALL
            .iter()
            .copied()
            .map(|role| TargetServiceBinding::new(role, callables.insert(())))
            .collect::<Vec<_>>();
        bindings.reverse();
        let registry = TargetServiceRegistry::new(bindings).unwrap();
        assert!(
            registry
                .bindings()
                .iter()
                .map(TargetServiceBinding::role)
                .eq(TargetServiceRole::ALL.iter().copied())
        );
        assert_eq!(
            TargetServiceRole::DarwinGetAddressInfo
                .descriptor()
                .signature()
                .parameters(),
            &[
                TargetServiceValueAbi::Pointer,
                TargetServiceValueAbi::Pointer,
                TargetServiceValueAbi::Pointer,
                TargetServiceValueAbi::Pointer,
            ]
        );
        assert_eq!(
            TargetServiceRole::DarwinGetAddressInfo
                .descriptor()
                .signature()
                .result(),
            Some(TargetServiceValueAbi::CInt)
        );
        let descriptor = registry.bindings()[0].descriptor();
        assert_eq!(descriptor.target(), CompilationTarget::Arm64Darwin);
        assert_eq!(
            descriptor.calling_convention(),
            TargetServiceCallingConvention::PlatformC
        );
        assert_eq!(
            descriptor.import().library(),
            RuntimeLibraryIdentity::DarwinSystem
        );
        assert_eq!(descriptor.import().symbol(), "_getaddrinfo");

        let empty = TargetServiceRegistry::empty();
        assert!(empty.bindings().is_empty());
        assert!(empty.binding(registry.bindings()[0].callable()).is_none());

        let missing = registry.bindings()[..1].iter().cloned();
        assert_eq!(
            TargetServiceRegistry::new(missing),
            Err(TargetServiceBindingError::MissingRole(
                TargetServiceRole::DarwinFreeAddressInfo
            ))
        );
    }
}
