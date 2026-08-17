use nocter_declarations::{
    FieldDeclaration, NominalTypeDeclaration, Parameter, StandardDeclarationRole,
    VariantDeclaration,
};
use nocter_model::{
    CaptureId, ExecutableItemId, FieldId, NominalTypeId, ParameterId, TypeId, TypeStore, VariantId,
};
use nocter_target_program::{ExecutableClosureLayout, ExecutableProgram};

/// The immutable semantic authority required to validate one MIR function.
///
/// Keeping this interface smaller than `ExecutableProgram` lets validation run per function in
/// future incremental pipelines without granting MIR access to source or package setup state.
pub trait MirValidationEnvironment {
    fn types(&self) -> &TypeStore;
    fn contains_item(&self, item: ExecutableItemId) -> bool;
    fn nominal_type(&self, id: NominalTypeId) -> Option<&NominalTypeDeclaration>;
    fn field(&self, id: FieldId) -> Option<&FieldDeclaration>;
    fn variant(&self, id: VariantId) -> Option<&VariantDeclaration>;
    fn parameter(&self, id: ParameterId) -> Option<&Parameter>;
    fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId>;
    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout>;
    fn closure_capture_type(&self, closure_ty: TypeId, capture: CaptureId) -> Option<TypeId>;
}

impl MirValidationEnvironment for ExecutableProgram {
    fn types(&self) -> &TypeStore {
        self.types()
    }

    fn contains_item(&self, item: ExecutableItemId) -> bool {
        self.items().get(item).is_some()
    }

    fn nominal_type(&self, id: NominalTypeId) -> Option<&NominalTypeDeclaration> {
        self.target()
            .checked()
            .graph()
            .declarations()
            .nominal_types()
            .get(id)
    }

    fn field(&self, id: FieldId) -> Option<&FieldDeclaration> {
        self.target()
            .checked()
            .graph()
            .declarations()
            .fields()
            .get(id)
    }

    fn variant(&self, id: VariantId) -> Option<&VariantDeclaration> {
        self.target()
            .checked()
            .graph()
            .declarations()
            .variants()
            .get(id)
    }

    fn parameter(&self, id: ParameterId) -> Option<&Parameter> {
        self.target()
            .checked()
            .graph()
            .declarations()
            .parameters()
            .get(id)
    }

    fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        self.target().checked().standard_semantics().nominal(role)
    }

    fn closure_layout(&self, item: ExecutableItemId) -> Option<&ExecutableClosureLayout> {
        self.closure_layout(item)
    }

    fn closure_capture_type(&self, closure_ty: TypeId, capture: CaptureId) -> Option<TypeId> {
        self.closure_capture_type(closure_ty, capture)
    }
}
