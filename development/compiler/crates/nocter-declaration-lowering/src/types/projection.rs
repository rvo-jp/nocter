use nocter_declarations::ExportedEntity;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::{SyntaxToken, SyntaxTree};

use crate::PreparedNamespaces;

use super::TypeBindingError;

pub(super) fn reference(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    entity: ExportedEntity,
    token: SyntaxToken,
) -> Result<(), TypeBindingError> {
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_index
        .insert(
            semantic_entity(entity),
            SourceRole::Reference,
            SourceOrigin::from_token(tree, token)
                .map_err(|_| TypeBindingError::InconsistentSource(tree.source()))?,
        )?;
    Ok(())
}

pub(super) fn generic(
    namespaces: &mut PreparedNamespaces<'_>,
    tree: &SyntaxTree,
    parameter: nocter_model::GenericParameterId,
    token: SyntaxToken,
) -> Result<(), TypeBindingError> {
    namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_index
        .insert(
            SemanticEntity::GenericParameter(parameter),
            SourceRole::Reference,
            SourceOrigin::from_token(tree, token)
                .map_err(|_| TypeBindingError::InconsistentSource(tree.source()))?,
        )?;
    Ok(())
}

const fn semantic_entity(entity: ExportedEntity) -> SemanticEntity {
    match entity {
        ExportedEntity::Module(id) => SemanticEntity::Module(id),
        ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}
