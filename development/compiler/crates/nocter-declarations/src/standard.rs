use std::collections::BTreeMap;

use nocter_model::{
    AssociatedTypeId, BuiltinType, CallableId, InterfaceId, ModuleId, NominalTypeId, PackageId,
};
use nocter_toolchain_contract::{StandardDeclarationRole, StructuralAttachment};

/// Exact declaration identity assigned one compiler-defined standard role during lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardDeclaration {
    BuiltinType(BuiltinType),
    NominalType(NominalTypeId),
    Interface(InterfaceId),
    AssociatedType(AssociatedTypeId),
    Callable(CallableId),
}

/// Exact compiler-selected authority for standard-library-only declarations.
#[derive(Debug)]
pub struct StandardLibrary {
    package: PackageId,
    structural_attachment_modules: BTreeMap<StructuralAttachment, ModuleId>,
    builtin_type_modules: [Option<ModuleId>; BuiltinType::COUNT],
    declarations: BTreeMap<StandardDeclarationRole, StandardDeclaration>,
}

impl StandardLibrary {
    pub(crate) const fn new(package: PackageId) -> Self {
        Self {
            package,
            structural_attachment_modules: BTreeMap::new(),
            builtin_type_modules: [None; BuiltinType::COUNT],
            declarations: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn builtin_type_module(&self, builtin: BuiltinType) -> Option<ModuleId> {
        self.builtin_type_modules[builtin.index()]
    }

    pub(crate) fn set_builtin_type_module(
        &mut self,
        builtin: BuiltinType,
        module: ModuleId,
    ) -> Result<(), ModuleId> {
        let slot = &mut self.builtin_type_modules[builtin.index()];
        match *slot {
            None => {
                *slot = Some(module);
                Ok(())
            }
            Some(existing) if existing == module => Ok(()),
            Some(existing) => Err(existing),
        }
    }

    #[must_use]
    pub const fn package(&self) -> PackageId {
        self.package
    }

    /// Returns the declaration selected for one compiler-defined standard role.
    #[must_use]
    pub fn declaration(&self, role: StandardDeclarationRole) -> Option<StandardDeclaration> {
        self.declarations.get(&role).copied()
    }

    /// Enumerates every role selected by the active toolchain in stable role order.
    #[must_use]
    pub fn declarations(
        &self,
    ) -> impl ExactSizeIterator<Item = (StandardDeclarationRole, StandardDeclaration)> + '_ {
        self.declarations
            .iter()
            .map(|(role, value)| (*role, *value))
    }

    pub(crate) fn set_declaration(
        &mut self,
        role: StandardDeclarationRole,
        declaration: StandardDeclaration,
    ) -> Result<(), StandardDeclaration> {
        match self.declarations.insert(role, declaration) {
            None => Ok(()),
            Some(existing) if existing == declaration => Ok(()),
            Some(existing) => {
                self.declarations.insert(role, existing);
                Err(existing)
            }
        }
    }

    #[must_use]
    pub fn structural_attachment_module(
        &self,
        attachment: StructuralAttachment,
    ) -> Option<ModuleId> {
        self.structural_attachment_modules.get(&attachment).copied()
    }

    pub(crate) fn set_structural_attachment_module(
        &mut self,
        attachment: StructuralAttachment,
        module: ModuleId,
    ) -> Result<(), ModuleId> {
        match self
            .structural_attachment_modules
            .insert(attachment, module)
        {
            None => Ok(()),
            Some(existing) if existing == module => Ok(()),
            Some(existing) => {
                self.structural_attachment_modules
                    .insert(attachment, existing);
                Err(existing)
            }
        }
    }
}
