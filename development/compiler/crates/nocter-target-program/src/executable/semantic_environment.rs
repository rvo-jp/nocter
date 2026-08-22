use nocter_checking::CheckedBody;
use nocter_declarations::{
    CallableKind, FieldDeclaration, NominalTypeDeclaration, OpaqueTypeDeclaration, Parameter,
    StandardDeclarationRole, VariantDeclaration,
};
use nocter_model::{
    BodyId, ExecutableItemId, FieldId, NominalTypeId, OpaqueTypeId, ParameterId, TypeId, VariantId,
};

use super::{ExecutableItem, ExecutableItemKey, ExecutableProgram};

/// Deliberate semantic queries admitted to MIR lowering.
///
/// These methods hide how the executable closure retains checked semantics. MIR may inspect the
/// returned algebraic contracts, but cannot navigate target-program storage to obtain more facts.
impl ExecutableProgram {
    #[must_use]
    pub fn checked_body(&self, body: BodyId) -> Option<&CheckedBody> {
        self.target.checked().bodies().get(body)
    }

    #[must_use]
    pub fn item_accepts_allocation_override(&self, item: ExecutableItemId) -> bool {
        let Some(ExecutableItemKey::Callable(key)) = self.items.get(item).map(ExecutableItem::key)
        else {
            return false;
        };
        self.target
            .checked()
            .graph()
            .declarations()
            .callables()
            .get(key.callable())
            .is_some_and(|callable| matches!(callable.kind(), CallableKind::Literal(_)))
    }

    #[must_use]
    pub fn nominal_type(&self, id: NominalTypeId) -> Option<&NominalTypeDeclaration> {
        self.target
            .checked()
            .graph()
            .declarations()
            .nominal_types()
            .get(id)
    }

    #[must_use]
    pub fn opaque_type(&self, id: OpaqueTypeId) -> Option<&OpaqueTypeDeclaration> {
        self.target
            .checked()
            .graph()
            .declarations()
            .opaque_types()
            .get(id)
    }

    #[must_use]
    pub fn opaque_witness(&self, id: OpaqueTypeId) -> Option<TypeId> {
        self.target.checked().opaque_witnesses().get(id)
    }

    #[must_use]
    pub fn field(&self, id: FieldId) -> Option<&FieldDeclaration> {
        self.target
            .checked()
            .graph()
            .declarations()
            .fields()
            .get(id)
    }

    #[must_use]
    pub fn variant(&self, id: VariantId) -> Option<&VariantDeclaration> {
        self.target
            .checked()
            .graph()
            .declarations()
            .variants()
            .get(id)
    }

    #[must_use]
    pub fn parameter(&self, id: ParameterId) -> Option<&Parameter> {
        self.target
            .checked()
            .graph()
            .declarations()
            .parameters()
            .get(id)
    }

    #[must_use]
    pub fn standard_nominal(&self, role: StandardDeclarationRole) -> Option<NominalTypeId> {
        self.target.checked().standard_semantics().nominal(role)
    }
}
