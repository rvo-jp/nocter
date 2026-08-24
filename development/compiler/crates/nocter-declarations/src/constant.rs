use nocter_model::{ConstantValue, DeclarationSiteId, Symbol, TypeId};

/// One complete module constant after its contract and initializer have been joined and evaluated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstantDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    ty: TypeId,
    value: ConstantValue,
    target_gate: Option<Symbol>,
}

impl ConstantDeclaration {
    #[must_use]
    pub const fn new(
        site: DeclarationSiteId,
        name: Symbol,
        ty: TypeId,
        value: ConstantValue,
        target_gate: Option<Symbol>,
    ) -> Self {
        Self {
            site,
            name,
            ty,
            value,
            target_gate,
        }
    }

    #[must_use]
    pub const fn site(&self) -> DeclarationSiteId {
        self.site
    }

    #[must_use]
    pub const fn name(&self) -> Symbol {
        self.name
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn value(&self) -> &ConstantValue {
        &self.value
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<Symbol> {
        self.target_gate
    }
}
