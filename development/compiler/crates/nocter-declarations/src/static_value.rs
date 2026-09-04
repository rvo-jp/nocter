use nocter_model::{CompilationTarget, DeclarationSiteId, FrozenValue, Symbol, TypeId};

/// One complete immutable static after its contract and initializer have been joined and evaluated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    ty: TypeId,
    value: FrozenValue,
    target_gate: Option<CompilationTarget>,
}

impl StaticDeclaration {
    #[must_use]
    pub const fn new(
        site: DeclarationSiteId,
        name: Symbol,
        ty: TypeId,
        value: FrozenValue,
        target_gate: Option<CompilationTarget>,
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
    pub const fn value(&self) -> &FrozenValue {
        &self.value
    }

    #[must_use]
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}
