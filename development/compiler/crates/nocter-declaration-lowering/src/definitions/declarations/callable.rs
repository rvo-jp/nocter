use nocter_declarations::{CallableDeclaration, CallableKind, CallableOwner, LiteralShape};
use nocter_model::CallableId;
use nocter_syntax::{NodeKind, Punctuation, StringDelimiter, TokenKind};

use crate::{PreparedTypes, ReservedEntity, SurfaceDeclarationId, SurfaceDeclarationKind};

use super::super::allocation::{
    AllocatedHeaders, entity, name, site, surface_kind, surface_node, surface_owner,
};
use super::super::{HeaderDefinitionError, projection, syntax};
use super::{callable_guarantees, own_generics, provenance, target};

pub(super) fn define(
    types: &mut PreparedTypes<'_>,
    allocated: &AllocatedHeaders,
    declaration: SurfaceDeclarationId,
    id: CallableId,
) -> Result<(), HeaderDefinitionError> {
    let kind = callable_kind(types, declaration)?;
    let owner = callable_owner(types, declaration)?;
    let named = matches!(
        kind,
        CallableKind::Function
            | CallableKind::Primitive
            | CallableKind::Method
            | CallableKind::ConstructionFunction
    );
    let result = types
        .callable_results
        .get(declaration.index())
        .copied()
        .flatten()
        .ok_or(HeaderDefinitionError::MissingCallableResult(declaration))?;
    let (contract, provenance_annotation) = provenance::contract(
        types,
        declaration,
        kind,
        allocated.receivers[declaration.index()],
        &allocated.parameters[declaration.index()],
        result,
        allocated.bodies[declaration.index()],
    )?;
    let definition = CallableDeclaration::new(
        site(types, declaration)?,
        owner,
        kind,
        named.then(|| name(types, declaration)).transpose()?,
        allocated.receivers[declaration.index()],
        own_generics(types, declaration),
        allocated.parameters[declaration.index()].clone(),
        result,
        callable_guarantees(types, declaration)?,
        contract,
        provenance_annotation,
        allocated.requirements[declaration.index()].clone(),
        allocated.bodies[declaration.index()],
        target::gate(types, declaration),
    );
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .define_callable(id, definition)?;
    Ok(())
}

fn callable_owner(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<CallableOwner, HeaderDefinitionError> {
    match surface_kind(types, declaration)? {
        SurfaceDeclarationKind::Function | SurfaceDeclarationKind::PrimitiveFunction => {
            let surface = types
                .namespaces
                .imports
                .generics
                .headers
                .reserved
                .declarations[declaration.index()];
            types
                .namespaces
                .imports
                .generics
                .headers
                .reserved
                .module_for_source(surface.source())
                .map(CallableOwner::Module)
                .ok_or(HeaderDefinitionError::InvalidOwner(declaration))
        }
        _ => {
            let owner = surface_owner(types, declaration)?;
            match entity(types, owner) {
                Some(ReservedEntity::Construction(owner)) => Ok(CallableOwner::Construction(owner)),
                Some(ReservedEntity::Instance(owner)) => Ok(CallableOwner::Instance(owner)),
                Some(ReservedEntity::Interface(owner)) => Ok(CallableOwner::Interface(owner)),
                _ => Err(HeaderDefinitionError::InvalidOwner(declaration)),
            }
        }
    }
}

fn callable_kind(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<CallableKind, HeaderDefinitionError> {
    Ok(match surface_kind(types, declaration)? {
        SurfaceDeclarationKind::Function => CallableKind::Function,
        SurfaceDeclarationKind::PrimitiveFunction => CallableKind::Primitive,
        SurfaceDeclarationKind::InterfaceMethod | SurfaceDeclarationKind::InherentMethod => {
            CallableKind::Method
        }
        SurfaceDeclarationKind::ConstructionFunction => CallableKind::ConstructionFunction,
        SurfaceDeclarationKind::Literal => {
            CallableKind::Literal(literal_shape(types, declaration)?)
        }
        SurfaceDeclarationKind::Coercion => CallableKind::Coercion,
        SurfaceDeclarationKind::Equality => CallableKind::Equality,
        SurfaceDeclarationKind::Ordering => CallableKind::Ordering,
        SurfaceDeclarationKind::Index => CallableKind::Index,
        SurfaceDeclarationKind::Expansion => CallableKind::Expansion,
        _ => return Err(HeaderDefinitionError::InvalidSurface(declaration)),
    })
}

fn literal_shape(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<LiteralShape, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    let shape = syntax::descendant(tree, root, NodeKind::LiteralShape)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    if syntax::has_punctuation(tree, shape, Punctuation::LeftBracket) {
        if syntax::has_punctuation(tree, shape, Punctuation::Colon) {
            Ok(LiteralShape::Mapping)
        } else {
            Ok(LiteralShape::Sequence)
        }
    } else if syntax::direct_tokens(tree, shape).into_iter().any(|token| {
        matches!(
            token.kind(),
            TokenKind::StringStart(StringDelimiter::SingleLine)
        )
    }) {
        Ok(LiteralShape::String)
    } else {
        Err(HeaderDefinitionError::InvalidSurface(declaration))
    }
}
