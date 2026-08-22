use nocter_declarations::{
    FieldDeclaration, NominalTypeDeclaration, OpaqueTypeDeclaration, Parameter,
    StandardDeclarationRole, VariantDeclaration,
};
use nocter_model::{
    CaptureId, ExecutableItemId, FieldId, NominalTypeId, OpaqueTypeId, ParameterId, TypeId,
    TypeStore, VariantId,
};
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
    fn nominal_type(&self, id: NominalTypeId) -> Option<&NominalTypeDeclaration>;
    fn opaque_type(&self, _id: OpaqueTypeId) -> Option<&OpaqueTypeDeclaration> {
        None
    }
    fn opaque_witness(&self, _id: OpaqueTypeId) -> Option<TypeId> {
        None
    }
    fn field(&self, id: FieldId) -> Option<&FieldDeclaration>;
    fn variant(&self, id: VariantId) -> Option<&VariantDeclaration>;
    fn parameter(&self, id: ParameterId) -> Option<&Parameter>;
    fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId>;
    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout>;
    fn closure_layout_for_type(&self, _ty: TypeId) -> Option<&ExecutableClosureLayout> {
        None
    }
    fn closure_capture_type(&self, closure_ty: TypeId, capture: CaptureId) -> Option<TypeId>;
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

    fn nominal_type(&self, id: NominalTypeId) -> Option<&NominalTypeDeclaration> {
        ExecutableProgram::nominal_type(self, id)
    }

    fn opaque_type(&self, id: OpaqueTypeId) -> Option<&OpaqueTypeDeclaration> {
        ExecutableProgram::opaque_type(self, id)
    }

    fn opaque_witness(&self, id: OpaqueTypeId) -> Option<TypeId> {
        ExecutableProgram::opaque_witness(self, id)
    }

    fn field(&self, id: FieldId) -> Option<&FieldDeclaration> {
        ExecutableProgram::field(self, id)
    }

    fn variant(&self, id: VariantId) -> Option<&VariantDeclaration> {
        ExecutableProgram::variant(self, id)
    }

    fn parameter(&self, id: ParameterId) -> Option<&Parameter> {
        ExecutableProgram::parameter(self, id)
    }

    fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        ExecutableProgram::standard_nominal(self, role)
    }

    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout> {
        self.closure_layout(item)
    }

    fn closure_layout_for_type(&self, ty: TypeId) -> Option<&ExecutableClosureLayout> {
        self.closure_layout_for_type(ty)
    }

    fn closure_capture_type(&self, closure_ty: TypeId, capture: CaptureId) -> Option<TypeId> {
        self.closure_capture_type(closure_ty, capture)
    }
}
