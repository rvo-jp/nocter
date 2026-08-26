use std::collections::BTreeMap;

use nocter_model::{ModuleId, Symbol};
use nocter_source_index::SyntaxOrigin;

use super::HeaderError;
use crate::{
    NamespaceViolation, ReservedDeclarations, ReservedEntity, SurfaceDeclaration,
    SurfaceDeclarationId, SurfaceDeclarationKind,
};

pub(super) fn resolve(
    reserved: &ReservedDeclarations<'_>,
) -> Result<Vec<Option<Symbol>>, HeaderError> {
    let mut names = Vec::with_capacity(reserved.declarations.len());
    for (index, declaration) in reserved.declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let name = resolve_name(reserved, id, declaration)?;
        names.push(name);
    }
    validate_joined_names(reserved, &names)?;
    validate_unique_names(reserved, &names)?;
    Ok(names)
}

fn resolve_name(
    reserved: &ReservedDeclarations<'_>,
    id: SurfaceDeclarationId,
    declaration: SurfaceDeclaration,
) -> Result<Option<Symbol>, HeaderError> {
    let Some(token) = declaration.name() else {
        return if requires_name(declaration.kind()) {
            Err(HeaderError::MissingName(id))
        } else {
            Ok(None)
        };
    };
    let source = reserved
        .source_map
        .get(token.source())
        .ok_or(HeaderError::MissingSource(declaration.source()))?;
    let spelling = source
        .text_at(token.range())
        .ok_or(HeaderError::InconsistentSource(token.source()))?;
    if spelling == "Self" {
        return Err(NamespaceViolation::reserved_name(SyntaxOrigin::Token(token)).into());
    }
    reserved
        .program
        .symbols()
        .get(spelling)
        .map(Some)
        .ok_or(HeaderError::MissingName(id))
}

fn validate_joined_names(
    reserved: &ReservedDeclarations<'_>,
    names: &[Option<Symbol>],
) -> Result<(), HeaderError> {
    for (index, name) in names.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        let representative = reserved.contracts.representative(id);
        if representative != id && names[representative.index()] != name {
            return Err(HeaderError::InconsistentName(id));
        }
    }
    Ok(())
}

fn validate_unique_names(
    reserved: &ReservedDeclarations<'_>,
    names: &[Option<Symbol>],
) -> Result<(), HeaderError> {
    let mut module_names = BTreeMap::<(ModuleId, Symbol), SurfaceDeclarationId>::new();
    let mut member_names = BTreeMap::<(ReservedEntity, Symbol), SurfaceDeclarationId>::new();
    let mut test_names = BTreeMap::<(ModuleId, Symbol), SurfaceDeclarationId>::new();
    for (index, declaration) in reserved.declarations.iter().copied().enumerate() {
        let id = SurfaceDeclarationId::from_index(index);
        if reserved.contracts.representative(id) != id {
            continue;
        }
        let Some(name) = names[index] else {
            continue;
        };
        if declaration.kind() == SurfaceDeclarationKind::Test {
            let module = module(reserved, declaration)?;
            if let Err(first) = insert_unique(&mut test_names, (module, name), id) {
                return Err(duplicate_name(reserved, first, id)?.into());
            }
        } else if let Some(owner) = declaration.owner() {
            let owner = reserved
                .entity(reserved.contracts.representative(owner))
                .ok_or(HeaderError::MissingName(id))?;
            if let Err(first) = insert_unique(&mut member_names, (owner, name), id) {
                return Err(duplicate_name(reserved, first, id)?.into());
            }
        } else if occupies_module_namespace(declaration.kind()) {
            if declaration.kind() != SurfaceDeclarationKind::PrimitiveType {
                reject_builtin_name(reserved, id, name)?;
            }
            let module = module(reserved, declaration)?;
            if let Err(first) = insert_unique(&mut module_names, (module, name), id) {
                return Err(duplicate_name(reserved, first, id)?.into());
            }
        }
    }
    Ok(())
}

fn reject_builtin_name(
    reserved: &ReservedDeclarations<'_>,
    declaration: SurfaceDeclarationId,
    name: Symbol,
) -> Result<(), HeaderError> {
    let spelling = reserved
        .program
        .symbols()
        .spelling(name)
        .ok_or(HeaderError::MissingName(declaration))?;
    if nocter_syntax::BuiltinType::from_spelling(spelling).is_some() {
        Err(NamespaceViolation::reserved_name(name_origin(reserved, declaration)?).into())
    } else {
        Ok(())
    }
}

fn duplicate_name(
    reserved: &ReservedDeclarations<'_>,
    first: SurfaceDeclarationId,
    second: SurfaceDeclarationId,
) -> Result<NamespaceViolation, HeaderError> {
    Ok(NamespaceViolation::name_collision(
        name_origin(reserved, first)?,
        name_origin(reserved, second)?,
    ))
}

fn name_origin(
    reserved: &ReservedDeclarations<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SyntaxOrigin, HeaderError> {
    reserved.declarations[declaration.index()]
        .name()
        .map(SyntaxOrigin::Token)
        .ok_or(HeaderError::MissingName(declaration))
}

fn module(
    reserved: &ReservedDeclarations<'_>,
    declaration: SurfaceDeclaration,
) -> Result<ModuleId, HeaderError> {
    reserved
        .module_for_source(declaration.source())
        .ok_or(HeaderError::MissingSource(declaration.source()))
}

fn insert_unique<K: Ord>(
    names: &mut BTreeMap<K, SurfaceDeclarationId>,
    key: K,
    declaration: SurfaceDeclarationId,
) -> Result<(), SurfaceDeclarationId> {
    if let Some(first) = names.insert(key, declaration) {
        Err(first)
    } else {
        Ok(())
    }
}

const fn occupies_module_namespace(kind: SurfaceDeclarationKind) -> bool {
    matches!(
        kind,
        SurfaceDeclarationKind::Constant
            | SurfaceDeclarationKind::Function
            | SurfaceDeclarationKind::PrimitiveFunction
            | SurfaceDeclarationKind::PrimitiveType
            | SurfaceDeclarationKind::TypeAlias
            | SurfaceDeclarationKind::Struct
            | SurfaceDeclarationKind::Enum
            | SurfaceDeclarationKind::Interface
    )
}

const fn requires_name(kind: SurfaceDeclarationKind) -> bool {
    matches!(
        kind,
        SurfaceDeclarationKind::Constant
            | SurfaceDeclarationKind::Function
            | SurfaceDeclarationKind::PrimitiveFunction
            | SurfaceDeclarationKind::PrimitiveType
            | SurfaceDeclarationKind::TypeAlias
            | SurfaceDeclarationKind::Struct
            | SurfaceDeclarationKind::Field
            | SurfaceDeclarationKind::Enum
            | SurfaceDeclarationKind::Variant
            | SurfaceDeclarationKind::Interface
            | SurfaceDeclarationKind::AssociatedType
            | SurfaceDeclarationKind::InterfaceMethod
            | SurfaceDeclarationKind::ConstructionFunction
            | SurfaceDeclarationKind::InherentMethod
            | SurfaceDeclarationKind::Test
    )
}
