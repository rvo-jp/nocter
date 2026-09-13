use nocter_checking::CheckedBody;
use nocter_model::{BodyId, ExecutableItemId, NominalTypeId, TypeId};
use nocter_runtime_contract::{RuntimeType, RuntimeTypeRepresentation};

use super::{ExecutableItem, ExecutableProgram};

/// Frozen semantic identities needed while lowering and validating MIR.
///
/// Keeping this small contract beside the executable closure prevents downstream layers from
/// reopening standard-library declaration state merely to recover well-known nominal identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ExecutableSemanticEnvironment {
    allocation_context: Option<NominalTypeId>,
    aborting_allocator: Option<NominalTypeId>,
}

impl ExecutableSemanticEnvironment {
    pub(super) fn freeze(target: &crate::TargetProgram) -> Self {
        let semantics = target.checked().standard_semantics();
        Self {
            allocation_context: semantics
                .nominal(nocter_toolchain_contract::StandardDeclarationRole::AllocationContext),
            aborting_allocator: semantics
                .nominal(nocter_toolchain_contract::StandardDeclarationRole::AbortingAllocator),
        }
    }

    pub(super) const fn allocation_context(self) -> Option<NominalTypeId> {
        self.allocation_context
    }

    pub(super) const fn aborting_allocator(self) -> Option<NominalTypeId> {
        self.aborting_allocator
    }
}

/// Deliberate semantic queries admitted to MIR lowering.
///
/// These methods hide how the executable closure retains checked semantics. MIR may inspect the
/// returned algebraic contracts, but cannot navigate target-program storage to obtain more facts.
impl ExecutableProgram {
    #[must_use]
    pub fn checked_body(&self, body: BodyId) -> Option<&CheckedBody> {
        self.checked_bodies.get(&body)
    }

    #[must_use]
    pub fn item_accepts_allocation_override(&self, item: ExecutableItemId) -> bool {
        self.items
            .get(item)
            .is_some_and(ExecutableItem::accepts_allocation_override)
    }

    #[must_use]
    pub fn type_representation(&self, ty: TypeId) -> Option<&RuntimeTypeRepresentation> {
        self.runtime.type_representations().get(ty)
    }

    /// Returns the closed runtime classification selected for one semantic type.
    ///
    /// MIR uses this narrow query when the physical category itself changes lowering semantics,
    /// such as compiler-owned storage whose source declaration intentionally has no fields.
    #[must_use]
    pub fn runtime_type(&self, ty: TypeId) -> Option<&RuntimeType> {
        self.runtime.types().get(ty)
    }

    #[must_use]
    pub fn allocation_context_nominal(&self) -> Option<NominalTypeId> {
        self.semantic_environment.allocation_context()
    }

    #[must_use]
    pub fn aborting_allocator_nominal(&self) -> Option<NominalTypeId> {
        self.semantic_environment.aborting_allocator()
    }
}
