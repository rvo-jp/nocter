use nocter_model::{BodyId, CallableId, InterfaceId, ModuleId, NominalTypeId, Symbol, TypeAliasId};

use crate::Visibility;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExportedEntity {
    Module(ModuleId),
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    Callable(CallableId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImportScope {
    Module(ModuleId),
    Body(BodyId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ImportedName {
    exported_name: Symbol,
    local_name: Symbol,
    target: ExportedEntity,
}

impl ImportedName {
    #[must_use]
    pub const fn new(exported_name: Symbol, local_name: Symbol, target: ExportedEntity) -> Self {
        Self {
            exported_name,
            local_name,
            target,
        }
    }

    #[must_use]
    pub const fn exported_name(self) -> Symbol {
        self.exported_name
    }

    #[must_use]
    pub const fn local_name(self) -> Symbol {
        self.local_name
    }

    #[must_use]
    pub const fn target(self) -> ExportedEntity {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImportTarget {
    Namespace {
        module: ModuleId,
        local_name: Symbol,
    },
    Selected {
        module: ModuleId,
        names: Box<[ImportedName]>,
    },
}

impl ImportTarget {
    #[must_use]
    pub const fn module(&self) -> ModuleId {
        match self {
            Self::Namespace { module, .. } | Self::Selected { module, .. } => *module,
        }
    }

    #[must_use]
    pub const fn namespace_name(&self) -> Option<Symbol> {
        match self {
            Self::Namespace { local_name, .. } => Some(*local_name),
            Self::Selected { .. } => None,
        }
    }

    #[must_use]
    pub const fn selected_names(&self) -> Option<&[ImportedName]> {
        match self {
            Self::Namespace { .. } => None,
            Self::Selected { names, .. } => Some(names),
        }
    }
}

/// One resolved module import or re-export.
///
/// Same-module source composition is absent: after source loading, its declarations belong to the
/// shared module and its physical edge remains in source-side data rather than semantic lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDeclaration {
    scope: ImportScope,
    visibility: Visibility,
    target: ImportTarget,
}

impl ImportDeclaration {
    #[must_use]
    pub const fn new(scope: ImportScope, visibility: Visibility, target: ImportTarget) -> Self {
        Self {
            scope,
            visibility,
            target,
        }
    }

    #[must_use]
    pub const fn scope(&self) -> ImportScope {
        self.scope
    }

    #[must_use]
    pub const fn visibility(&self) -> Visibility {
        self.visibility
    }

    #[must_use]
    pub const fn target(&self) -> &ImportTarget {
        &self.target
    }
}
