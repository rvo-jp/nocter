use nocter_declarations::{DeclarationGraph, NominalShape};
use nocter_model::NominalTypeId;
use nocter_runtime_contract::{RuntimeStorageRegistry, RuntimeStorageRole};

/// Validates every compiler-owned storage binding against its closed semantic declaration.
pub(super) fn validate_runtime_storage(
    graph: &DeclarationGraph,
    registry: &RuntimeStorageRegistry,
) -> Result<(), RuntimeStorageContractError> {
    let standard = graph
        .standard_package()
        .ok_or(RuntimeStorageContractError::MissingStandardPackage)?;
    for binding in registry.bindings() {
        let declaration = graph
            .declarations()
            .nominal_types()
            .get(binding.declaration())
            .ok_or(RuntimeStorageContractError::MissingDeclaration {
                role: binding.role(),
                declaration: binding.declaration(),
            })?;
        let site = graph.declaration_sites().get(declaration.site()).ok_or(
            RuntimeStorageContractError::InvalidDeclaration(binding.declaration()),
        )?;
        let module = graph.modules().get(site.module()).ok_or(
            RuntimeStorageContractError::InvalidDeclaration(binding.declaration()),
        )?;
        let empty_unique = matches!(
            declaration.shape(),
            NominalShape::Struct {
                copy_declared: false,
                fields,
            } if fields.is_empty()
        );
        if module.package() != standard
            || !declaration.generic_parameters().is_empty()
            || !declaration.requirements().is_empty()
            || !empty_unique
        {
            return Err(RuntimeStorageContractError::InvalidDeclaration(
                binding.declaration(),
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeStorageContractError {
    MissingStandardPackage,
    MissingDeclaration {
        role: RuntimeStorageRole,
        declaration: NominalTypeId,
    },
    InvalidDeclaration(NominalTypeId),
}

impl std::fmt::Display for RuntimeStorageContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid runtime storage contract: {self:?}")
    }
}

impl std::error::Error for RuntimeStorageContractError {}
