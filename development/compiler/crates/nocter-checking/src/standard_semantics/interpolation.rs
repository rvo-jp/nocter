use nocter_declarations::{
    CallableKind, CallableOwner, DeclarationGraph, ParameterRole, StandardDeclarationRole,
    Visibility,
};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, CallableId, DeclarationSiteId,
    NominalTypeId, TypeId, TypeKind, TypeStore,
};

use super::StandardSemanticError;

/// Validates the complete standard-library surface required by string interpolation.
///
/// The checker freezes these callables into each checked interpolation. Keeping the contract here
/// prevents later stages from recovering operations from names or learning `String`'s layout.
pub(super) fn validate_interpolation_roles(
    graph: &DeclarationGraph,
    types: &TypeStore,
    string: Option<NominalTypeId>,
    constructor: Option<CallableId>,
    appender: Option<CallableId>,
) -> Result<(), StandardSemanticError> {
    if constructor.is_none() && appender.is_none() {
        return Ok(());
    }
    let owner_role = if constructor.is_some() {
        StandardDeclarationRole::InterpolationConstructor
    } else {
        StandardDeclarationRole::InterpolationTextAppender
    };
    let string = string.ok_or(StandardSemanticError::MissingDependency {
        role: owner_role,
        dependency: StandardDeclarationRole::OwnedString,
    })?;
    let constructor = constructor.ok_or(StandardSemanticError::MissingDependency {
        role: StandardDeclarationRole::InterpolationTextAppender,
        dependency: StandardDeclarationRole::InterpolationConstructor,
    })?;
    let appender = appender.ok_or(StandardSemanticError::MissingDependency {
        role: StandardDeclarationRole::InterpolationConstructor,
        dependency: StandardDeclarationRole::InterpolationTextAppender,
    })?;
    validate_constructor(graph, types, string, constructor)?;
    validate_appender(graph, types, string, appender)
}

fn validate_constructor(
    graph: &DeclarationGraph,
    types: &TypeStore,
    string: NominalTypeId,
    callable_id: CallableId,
) -> Result<(), StandardSemanticError> {
    let callable = graph
        .declarations()
        .callables()
        .get(callable_id)
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    let CallableOwner::Construction(owner) = callable.owner() else {
        return Err(StandardSemanticError::InvalidInterpolationContract);
    };
    let construction = graph
        .declarations()
        .constructions()
        .get(owner)
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    if callable.kind() != CallableKind::ConstructionFunction
        || !construction.members().contains(&callable_id)
        || callable.receiver().is_some()
        || !callable.parameters().is_empty()
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.body().is_none()
        || !is_public(graph, callable.site())
        || !is_owned_string_type(types, construction.target(), string)
        || !construction.generic_parameters().is_empty()
        || callable.result() != construction.target()
    {
        return Err(StandardSemanticError::InvalidInterpolationContract);
    }
    Ok(())
}

fn validate_appender(
    graph: &DeclarationGraph,
    types: &TypeStore,
    string: NominalTypeId,
    callable_id: CallableId,
) -> Result<(), StandardSemanticError> {
    let callable = graph
        .declarations()
        .callables()
        .get(callable_id)
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    let CallableOwner::Instance(owner) = callable.owner() else {
        return Err(StandardSemanticError::InvalidInterpolationContract);
    };
    let instance = graph
        .declarations()
        .instances()
        .get(owner)
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    let receiver = callable
        .receiver()
        .and_then(|receiver| graph.declarations().parameters().get(receiver))
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    let [text] = callable.parameters() else {
        return Err(StandardSemanticError::InvalidInterpolationContract);
    };
    let text = graph
        .declarations()
        .parameters()
        .get(*text)
        .ok_or(StandardSemanticError::InvalidInterpolationContract)?;
    if callable.kind() != CallableKind::Method
        || !instance.members().contains(&callable_id)
        || !instance.generic_parameters().is_empty()
        || !instance.requirements().is_empty()
        || !is_owned_string_type(types, instance.target(), string)
        || !callable.generic_parameters().is_empty()
        || !callable.requirements().is_empty()
        || callable.body().is_none()
        || !is_public(graph, callable.site())
        || receiver.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
        || receiver.ty() != instance.target()
        || text.role() != (ParameterRole::Ordinary { position: 0 })
        || !matches!(
            types.get(text.ty()),
            Some(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            }) if *referent == types.builtin(BuiltinType::Str)
        )
        || callable.result() != types.builtin(BuiltinType::Void)
    {
        return Err(StandardSemanticError::InvalidInterpolationContract);
    }
    Ok(())
}

fn is_owned_string_type(types: &TypeStore, ty: TypeId, string: NominalTypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Nominal {
            definition,
            arguments,
        }) if *definition == string && arguments.is_empty()
    )
}

fn is_public(graph: &DeclarationGraph, site: DeclarationSiteId) -> bool {
    graph
        .declaration_sites()
        .get(site)
        .is_some_and(|site| site.visibility() == Visibility::Public)
}
