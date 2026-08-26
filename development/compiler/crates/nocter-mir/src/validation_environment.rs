use nocter_model::{ExecutableItemId, NominalTypeId, TypeId, TypeStore};
use nocter_runtime_contract::RuntimeTypeRepresentation;
use nocter_target_program::{ExecutableClosureLayout, ExecutableProgram};

/// The immutable semantic authority required to validate one MIR function.
///
/// Keeping this interface smaller than `ExecutableProgram` lets validation run per function in
/// future incremental pipelines without granting MIR access to source or package setup state.
pub trait MirValidationEnvironment {
    fn types(&self) -> &TypeStore;
    fn contains_item(&self, item: ExecutableItemId) -> bool;
    fn item_pack_input(&self, _item: ExecutableItemId) -> Option<(TypeId, TypeId)> {
        None
    }
    fn item_accepts_allocation_override(&self, _item: ExecutableItemId) -> bool {
        false
    }
    fn type_representation(&self, ty: TypeId) -> Option<&RuntimeTypeRepresentation>;
    fn allocation_context_nominal(&self) -> Option<NominalTypeId>;
    fn aborting_allocator_nominal(&self) -> Option<NominalTypeId>;
    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout>;
}

impl MirValidationEnvironment for ExecutableProgram {
    fn types(&self) -> &TypeStore {
        self.types()
    }

    fn contains_item(&self, item: ExecutableItemId) -> bool {
        self.items().get(item).is_some()
    }

    fn item_pack_input(&self, item: ExecutableItemId) -> Option<(TypeId, TypeId)> {
        self.items()
            .get(item)
            .and_then(|item| item.signature().pack())
            .map(|pack| (pack.element(), pack.next()))
    }

    fn item_accepts_allocation_override(&self, item: ExecutableItemId) -> bool {
        ExecutableProgram::item_accepts_allocation_override(self, item)
    }

    fn type_representation(&self, ty: TypeId) -> Option<&RuntimeTypeRepresentation> {
        ExecutableProgram::type_representation(self, ty)
    }

    fn allocation_context_nominal(&self) -> Option<NominalTypeId> {
        ExecutableProgram::allocation_context_nominal(self)
    }

    fn aborting_allocator_nominal(&self) -> Option<NominalTypeId> {
        ExecutableProgram::aborting_allocator_nominal(self)
    }

    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout> {
        self.closure_layout(item)
    }
}
