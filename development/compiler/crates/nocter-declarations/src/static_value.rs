use nocter_model::{CompilationTarget, DeclarationSiteId, Symbol, TypeId};

/// One complete immutable static contract, separate from its evaluated value authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticDeclaration {
    site: DeclarationSiteId,
    name: Symbol,
    ty: TypeId,
    target_gate: Option<CompilationTarget>,
}

impl StaticDeclaration {
    #[must_use]
    pub const fn new(
        site: DeclarationSiteId,
        name: Symbol,
        ty: TypeId,
        target_gate: Option<CompilationTarget>,
    ) -> Self {
        Self {
            site,
            name,
            ty,
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
    pub const fn target_gate(&self) -> Option<CompilationTarget> {
        self.target_gate
    }
}
