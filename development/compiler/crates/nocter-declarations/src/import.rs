use nocter_model::{
    BuiltinType, CallableId, ConstantId, InterfaceId, ModuleId, NominalTypeId, Symbol, TypeAliasId,
};

use crate::Visibility;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExportedEntity {
    Module(ModuleId),
    BuiltinType(BuiltinType),
    NominalType(NominalTypeId),
    TypeAlias(TypeAliasId),
    Interface(InterfaceId),
    Callable(CallableId),
    Constant(ConstantId),
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

/// One resolved top-level module import or re-export.
///
/// Physical-source visibility is absent: each source-local namespace is prepared before semantic
/// imports, and its directed see edges remain in the frontend binding input. Block imports
/// belong to checked lexical scopes and do not enter this declaration arena.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDeclaration {
    module: ModuleId,
    visibility: Visibility,
    target: ImportTarget,
}

impl ImportDeclaration {
    #[must_use]
    pub const fn new(module: ModuleId, visibility: Visibility, target: ImportTarget) -> Self {
        Self {
            module,
            visibility,
            target,
        }
    }

    #[must_use]
    pub const fn module(&self) -> ModuleId {
        self.module
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
