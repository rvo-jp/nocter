use nocter_declarations::ExportedEntity;
use nocter_model::{ImportId, ModuleId};
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::{NodeId, SyntaxToken};

use super::{ImportError, ResolvedImport};
use crate::PreparedGenerics;

pub(super) fn project_import(
    generics: &mut PreparedGenerics<'_>,
    declaration: NodeId,
    path: NodeId,
    import: ImportId,
    module: ModuleId,
    resolved: &ResolvedImport,
) -> Result<(), ImportError> {
    let tree = generics
        .headers
        .reserved
        .sources
        .iter()
        .find(|candidate| candidate.syntax().source() == declaration.source())
        .map(crate::SurfaceSource::syntax)
        .ok_or(ImportError::InconsistentSource(declaration.source()))?;
    generics.headers.reserved.source_index.insert(
        SemanticEntity::Import(import),
        SourceRole::Declaration,
        SourceOrigin::from_node(tree, declaration)
            .map_err(|_| ImportError::InconsistentSource(declaration.source()))?,
    )?;
    generics.headers.reserved.source_index.insert(
        SemanticEntity::Module(module),
        SourceRole::Reference,
        SourceOrigin::from_node(tree, path)
            .map_err(|_| ImportError::InconsistentSource(declaration.source()))?,
    )?;
    if let ResolvedImport::Selected(names) = resolved {
        for name in names {
            project_reference(
                &mut generics.headers.reserved.source_index,
                tree,
                name.target,
                name.exported_token,
            )?;
            if name.local_token != name.exported_token {
                project_reference(
                    &mut generics.headers.reserved.source_index,
                    tree,
                    name.target,
                    name.local_token,
                )?;
            }
        }
    }
    Ok(())
}

fn project_reference(
    index: &mut nocter_source_index::SourceIndexBuilder,
    tree: &nocter_syntax::SyntaxTree,
    entity: ExportedEntity,
    token: SyntaxToken,
) -> Result<(), ImportError> {
    index.insert(
        semantic_entity(entity),
        SourceRole::Reference,
        SourceOrigin::from_token(tree, token)
            .map_err(|_| ImportError::InconsistentSource(tree.source()))?,
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
