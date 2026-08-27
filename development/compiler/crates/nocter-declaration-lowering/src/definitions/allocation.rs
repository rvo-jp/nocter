use nocter_declarations::{
    Body, BodyOwner, FieldDeclaration, Parameter, ParameterOwner, ParameterRole, Requirement,
    RequirementOwner,
};
use nocter_model::{
    BodyId, BorrowCapability, CallableCapability, CallableId, FieldId, ParameterId, RequirementId,
    TypeId, TypeKind, VariantId,
};
use nocter_source_index::{SemanticEntity, SourceRole};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxOrigin, SyntaxToken, TokenKind};

use crate::{
    LoweredDeclarations, PreparedTypes, ReservedEntity, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

use super::{HeaderDefinitionError, HeaderDefinitionFailure, projection, syntax};

#[derive(Debug)]
pub(super) struct AllocatedHeaders {
    pub(super) requirements: Box<[Box<[RequirementId]>]>,
    pub(super) fields: Box<[Option<FieldId>]>,
    pub(super) receivers: Box<[Option<ParameterId>]>,
    pub(super) parameters: Box<[Box<[ParameterId]>]>,
    pub(super) bodies: Box<[Option<BodyId>]>,
}

#[derive(Clone, Copy)]
struct ParameterSyntax {
    name: SyntaxToken,
    ty: TypeId,
    role: ParameterRole,
}

pub(super) fn allocate(
    types: &mut PreparedTypes<'_>,
) -> Result<AllocatedHeaders, HeaderDefinitionError> {
    let count = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .len();
    let mut requirements = vec![Box::<[RequirementId]>::default(); count];
    let mut fields = vec![None; count];
    let mut receivers = vec![None; count];
    let mut parameters = vec![Box::<[ParameterId]>::default(); count];
    let mut bodies = vec![None; count];

    for index in 0..count {
        let declaration = SurfaceDeclarationId::from_index(index);
        if representative(types, declaration) != declaration {
            continue;
        }
        requirements[index] = allocate_requirements(types, declaration)?;
        match entity(types, declaration) {
            Some(ReservedEntity::Callable(callable)) => {
                let (receiver, ordinary) =
                    allocate_callable_parameters(types, declaration, callable)?;
                receivers[index] = receiver;
                parameters[index] = ordinary;
                bodies[index] = allocate_callable_body(types, declaration, callable)?;
            }
            Some(ReservedEntity::Variant(variant)) => {
                parameters[index] = allocate_variant_parameters(types, declaration, variant)?;
            }
            Some(ReservedEntity::Drop(drop)) => {
                let target = pattern_type(types, declaration, 0)?;
                let name = drop_receiver_token(types, declaration)?;
                let symbol = projection::symbol(types, declaration, name)?;
                let receiver = types
                    .namespaces
                    .imports
                    .generics
                    .headers
                    .reserved
                    .program
                    .declarations_mut()
                    .add_parameter(Parameter::new(
                        ParameterOwner::Drop(drop),
                        symbol,
                        target,
                        ParameterRole::Receiver(CallableCapability::ReadWrite),
                    ));
                projection::parameter(types, declaration, receiver, SourceRole::Declaration, name)?;
                receivers[index] = Some(receiver);
                bodies[index] = allocate_body(types, declaration, BodyOwner::Drop(drop))?;
            }
            Some(ReservedEntity::Test(test)) => {
                bodies[index] = allocate_body(types, declaration, BodyOwner::Test(test))?;
            }
            _ => {}
        }

        if surface_kind(types, declaration)? == SurfaceDeclarationKind::Field {
            fields[index] = Some(allocate_field(types, declaration)?);
        }
    }

    Ok(AllocatedHeaders {
        requirements: requirements.into_boxed_slice(),
        fields: fields.into_boxed_slice(),
        receivers: receivers.into_boxed_slice(),
        parameters: parameters.into_boxed_slice(),
        bodies: bodies.into_boxed_slice(),
    })
}

pub(super) fn finish_recovering(
    mut types: PreparedTypes<'_>,
    _allocated: AllocatedHeaders,
) -> Result<LoweredDeclarations, HeaderDefinitionFailure> {
    project_associated_projection_uses(&mut types)
        .map_err(HeaderDefinitionFailure::without_recovery)?;
    types
        .namespaces
        .define_program_namespaces()
        .map_err(|error| match error {
            crate::imports::NamespaceDefinitionError::Program(error) => {
                HeaderDefinitionError::Program(error)
            }
            crate::imports::NamespaceDefinitionError::FrontendBindings(error) => {
                HeaderDefinitionError::FrontendBindings(error)
            }
        })
        .map_err(HeaderDefinitionFailure::without_recovery)?;
    let reserved = types.namespaces.imports.generics.headers.reserved;
    let primitive_bindings = reserved.primitive_bindings;
    let (source_index, frontend_bindings) = reserved.source_index.finish();
    match reserved.program.finish_recovering() {
        Ok(program) => Ok(LoweredDeclarations::new(
            program,
            frontend_bindings,
            source_index,
            primitive_bindings,
        )),
        Err(failure) => {
            let (error, recovery) = match failure {
                nocter_declarations::ProgramBuildFailure::Rejected(rejected) => {
                    let (report, analysis) = rejected.into_analysis();
                    match super::DeclarationDiagnostics::project(&report, &source_index) {
                        Ok(diagnostics) => {
                            let recovery = crate::DeclarationLoweringRecovery::new(
                                analysis,
                                frontend_bindings,
                                source_index,
                            );
                            (
                                HeaderDefinitionError::Declaration(diagnostics),
                                Some(recovery),
                            )
                        }
                        Err(subject) => (
                            HeaderDefinitionError::MissingDiagnosticSubject(subject),
                            None,
                        ),
                    }
                }
                nocter_declarations::ProgramBuildFailure::Error(error) => {
                    (HeaderDefinitionError::Program(error), None)
                }
            };
            Err(HeaderDefinitionFailure::new(error, recovery))
        }
    }
}

fn project_associated_projection_uses(
    types: &mut PreparedTypes<'_>,
) -> Result<(), HeaderDefinitionError> {
    let uses = std::mem::take(&mut types.associated_projection_uses).into_vec();
    for projection in uses {
        let syntax = projection.origin();
        let source = match syntax {
            SyntaxOrigin::Node(node) => node.source(),
            SyntaxOrigin::Token(token) => token.source(),
        };
        let tree = types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .sources
            .iter()
            .find(|candidate| candidate.syntax().source() == source)
            .map(crate::SurfaceSource::syntax)
            .ok_or(HeaderDefinitionError::InconsistentSource(source))?;
        let origin = match syntax {
            SyntaxOrigin::Node(node) => nocter_source_index::SourceOrigin::from_node(tree, node)
                .map_err(|_| HeaderDefinitionError::InconsistentSource(source))?,
            SyntaxOrigin::Token(token) => {
                nocter_source_index::SourceOrigin::from_token(tree, token)
                    .map_err(|_| HeaderDefinitionError::InconsistentSource(source))?
            }
        };
        types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .source_index
            .insert_associated_projection_use(
                projection.base(),
                projection.associated(),
                syntax,
                origin,
            );
    }
    Ok(())
}

fn allocate_requirements(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<Box<[RequirementId]>, HeaderDefinitionError> {
    let kinds = types
        .requirements
        .get(declaration.index())
        .cloned()
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    if kinds.is_empty() {
        return Ok(Box::new([]));
    }
    let owner = requirement_owner(types, declaration)?;
    let mut ids = Vec::with_capacity(kinds.len());
    for kind in kinds {
        ids.push(
            types
                .namespaces
                .imports
                .generics
                .headers
                .reserved
                .program
                .declarations_mut()
                .add_requirement(Requirement::new(owner, kind)),
        );
    }
    for index in 0..surface_count(types) {
        let occurrence = SurfaceDeclarationId::from_index(index);
        if representative(types, occurrence) != declaration {
            continue;
        }
        let tree = projection::tree(types, occurrence)?;
        let root = surface_node(types, occurrence)?;
        let origins = syntax::requirement_origins(tree, root);
        if origins.len() != ids.len() {
            return Err(HeaderDefinitionError::InvalidSurface(occurrence));
        }
        for (requirement, node) in ids.iter().copied().zip(origins) {
            projection::node(
                types,
                occurrence,
                SemanticEntity::Requirement(requirement),
                projection::role(types, occurrence),
                node,
            )?;
        }
    }
    Ok(ids.into_boxed_slice())
}

fn allocate_field(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<FieldId, HeaderDefinitionError> {
    let owner = surface_owner(types, declaration)?;
    let Some(ReservedEntity::NominalType(owner)) = entity(types, owner) else {
        return Err(HeaderDefinitionError::InvalidOwner(declaration));
    };
    let site = site(types, declaration)?;
    let name = name(types, declaration)?;
    let tree = projection::tree(types, declaration)?;
    let node = surface_node(types, declaration)?;
    let ty_node = syntax::direct_node(tree, node, NodeKind::Type)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    let ty = normalized_type(types, ty_node)?;
    let field = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .add_field(FieldDeclaration::new(site, owner, name, ty));
    let token = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations[declaration.index()]
    .name()
    .ok_or(HeaderDefinitionError::MissingName(declaration))?;
    projection::token(
        types,
        declaration,
        SemanticEntity::Field(field),
        SourceRole::Declaration,
        token,
    )?;
    projection::documentation(types, declaration, SemanticEntity::Field(field))?;
    Ok(field)
}

fn allocate_variant_parameters(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    variant: VariantId,
) -> Result<Box<[ParameterId]>, HeaderDefinitionError> {
    let syntax = ordinary_parameter_syntax(types, declaration, SurfaceDeclarationKind::Variant)?;
    allocate_parameters(
        types,
        declaration,
        ParameterOwner::Variant(variant),
        &syntax,
    )
}

fn allocate_callable_parameters(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    callable: CallableId,
) -> Result<(Option<ParameterId>, Box<[ParameterId]>), HeaderDefinitionError> {
    let kind = surface_kind(types, declaration)?;
    let receiver = receiver_syntax(types, declaration, kind)?;
    let ordinary = ordinary_parameter_syntax(types, declaration, kind)?;
    let receiver_id = receiver
        .map(|syntax| {
            allocate_parameter(
                types,
                declaration,
                ParameterOwner::Callable(callable),
                syntax,
            )
        })
        .transpose()?;
    let parameters = allocate_parameters(
        types,
        declaration,
        ParameterOwner::Callable(callable),
        &ordinary,
    )?;

    for index in 0..surface_count(types) {
        let occurrence = SurfaceDeclarationId::from_index(index);
        if occurrence == declaration || representative(types, occurrence) != declaration {
            continue;
        }
        let occurrence_receiver = receiver_syntax(types, occurrence, kind)?;
        match (receiver_id, occurrence_receiver) {
            (Some(parameter), Some(syntax)) => {
                project_parameter(types, occurrence, parameter, syntax.name)?;
            }
            (None, None) => {}
            _ => return Err(HeaderDefinitionError::InvalidSurface(occurrence)),
        }
        let occurrence_parameters = ordinary_parameter_syntax(types, occurrence, kind)?;
        if occurrence_parameters.len() != parameters.len() {
            return Err(HeaderDefinitionError::InvalidSurface(occurrence));
        }
        for (parameter, syntax) in parameters.iter().copied().zip(occurrence_parameters) {
            project_parameter(types, occurrence, parameter, syntax.name)?;
        }
    }
    Ok((receiver_id, parameters))
}

fn allocate_parameters(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    owner: ParameterOwner,
    syntax: &[ParameterSyntax],
) -> Result<Box<[ParameterId]>, HeaderDefinitionError> {
    syntax
        .iter()
        .copied()
        .map(|syntax| allocate_parameter(types, declaration, owner, syntax))
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn allocate_parameter(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    owner: ParameterOwner,
    syntax: ParameterSyntax,
) -> Result<ParameterId, HeaderDefinitionError> {
    let name = projection::symbol(types, declaration, syntax.name)?;
    let parameter = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .add_parameter(Parameter::new(owner, name, syntax.ty, syntax.role));
    project_parameter(types, declaration, parameter, syntax.name)?;
    Ok(parameter)
}

fn project_parameter(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    parameter: ParameterId,
    token: SyntaxToken,
) -> Result<(), HeaderDefinitionError> {
    projection::parameter(
        types,
        declaration,
        parameter,
        projection::role(types, declaration),
        token,
    )
}

fn ordinary_parameter_syntax(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    kind: SurfaceDeclarationKind,
) -> Result<Vec<ParameterSyntax>, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    if matches!(
        kind,
        SurfaceDeclarationKind::Equality | SurfaceDeclarationKind::Ordering
    ) {
        let name = syntax::direct_identifier(tree, root)
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
        let referent = owner_self_type(types, declaration)?;
        let ty = types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .program
            .types_mut()
            .intern(TypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            })
            .map_err(|_| HeaderDefinitionError::InvalidTypePattern(declaration))?;
        return Ok(vec![ParameterSyntax {
            name,
            ty,
            role: ParameterRole::Ordinary { position: 0 },
        }]);
    }
    if kind == SurfaceDeclarationKind::Index {
        let parameter = syntax::descendant(tree, root, NodeKind::Parameter)
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
        return Ok(vec![ordinary_parameter(
            types,
            declaration,
            tree,
            parameter,
            0,
        )?]);
    }
    let Some(parameters) = syntax::descendant(tree, root, NodeKind::Parameters) else {
        return Ok(Vec::new());
    };
    let parameter_nodes = syntax::direct_nodes(tree, parameters, NodeKind::Parameter);
    validate_argument_pack_shape(tree, root, parameters, kind, &parameter_nodes)?;
    parameter_nodes
        .into_iter()
        .enumerate()
        .map(|(position, parameter)| {
            ordinary_parameter(types, declaration, tree, parameter, position)
        })
        .collect()
}

fn validate_argument_pack_shape(
    tree: &nocter_syntax::SyntaxTree,
    root: NodeId,
    parameters: NodeId,
    kind: SurfaceDeclarationKind,
    parameter_nodes: &[NodeId],
) -> Result<(), HeaderDefinitionError> {
    let packs = parameter_nodes
        .iter()
        .copied()
        .filter_map(|parameter| {
            syntax::direct_node(tree, parameter, NodeKind::ArgumentPackModifier)
        })
        .collect::<Vec<_>>();
    let one_final = packs.len() == 1
        && parameter_nodes.last().is_some_and(|parameter| {
            syntax::direct_node(tree, *parameter, NodeKind::ArgumentPackModifier)
                == packs.first().copied()
        });
    let valid = match kind {
        SurfaceDeclarationKind::Literal => {
            let sequence =
                syntax::descendant(tree, root, NodeKind::LiteralShape).is_some_and(|shape| {
                    syntax::has_punctuation(tree, shape, Punctuation::LeftBracket)
                });
            if sequence {
                one_final && parameter_nodes.len() == 1
            } else {
                packs.is_empty()
            }
        }
        SurfaceDeclarationKind::Function
        | SurfaceDeclarationKind::ConstructionFunction
        | SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::InherentMethod => packs.is_empty() || one_final,
        _ => packs.is_empty(),
    };
    if valid {
        return Ok(());
    }
    Err(super::DefinitionViolation::new(
        super::DefinitionRule::InvalidArgumentPackParameter,
        SyntaxOrigin::Node(packs.first().copied().unwrap_or(parameters)),
    )
    .into())
}

fn ordinary_parameter(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    tree: &nocter_syntax::SyntaxTree,
    node: NodeId,
    position: usize,
) -> Result<ParameterSyntax, HeaderDefinitionError> {
    let name = syntax::direct_identifier(tree, node)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    let ty_node = syntax::direct_node(tree, node, NodeKind::Type)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    Ok(ParameterSyntax {
        name,
        ty: normalized_type(types, ty_node)?,
        role: if syntax::direct_node(tree, node, NodeKind::ArgumentPackModifier).is_some() {
            ParameterRole::ArgumentPack { position }
        } else {
            ParameterRole::Ordinary { position }
        },
    })
}

fn receiver_syntax(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    kind: SurfaceDeclarationKind,
) -> Result<Option<ParameterSyntax>, HeaderDefinitionError> {
    let capability = match kind {
        SurfaceDeclarationKind::InterfaceMethod
        | SurfaceDeclarationKind::InherentMethod
        | SurfaceDeclarationKind::Coercion
        | SurfaceDeclarationKind::Equality
        | SurfaceDeclarationKind::Ordering
        | SurfaceDeclarationKind::Index
        | SurfaceDeclarationKind::Expansion => Some(receiver_capability(types, declaration)?),
        _ => None,
    };
    let Some(capability) = capability else {
        return Ok(None);
    };
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    let receiver = syntax::descendant(tree, root, NodeKind::Receiver)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    let name = syntax::direct_identifier(tree, receiver)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    Ok(Some(ParameterSyntax {
        name,
        ty: owner_self_type(types, declaration)?,
        role: ParameterRole::Receiver(capability),
    }))
}

fn receiver_capability(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<CallableCapability, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    let receiver = syntax::descendant(tree, root, NodeKind::Receiver)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    Ok(
        if syntax::has_punctuation(tree, receiver, Punctuation::ReadWrite) {
            CallableCapability::ReadWrite
        } else if syntax::has_punctuation(tree, receiver, Punctuation::Ampersand) {
            CallableCapability::Readonly
        } else {
            CallableCapability::Owned
        },
    )
}

fn owner_self_type(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<TypeId, HeaderDefinitionError> {
    let owner = surface_owner(types, declaration)?;
    match entity(types, owner) {
        Some(ReservedEntity::Interface(interface)) => types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .program
            .types_mut()
            .intern(TypeKind::InterfaceSelf(interface))
            .map_err(|_| HeaderDefinitionError::InvalidTypePattern(owner)),
        Some(ReservedEntity::Instance(_)) => pattern_type(types, owner, 0),
        Some(ReservedEntity::InterfaceImplementation(_)) => pattern_type(types, owner, 1),
        _ => Err(HeaderDefinitionError::InvalidOwner(declaration)),
    }
}

fn allocate_callable_body(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    callable: CallableId,
) -> Result<Option<BodyId>, HeaderDefinitionError> {
    let mut found = None;
    for index in 0..surface_count(types) {
        let occurrence = SurfaceDeclarationId::from_index(index);
        if representative(types, occurrence) != declaration {
            continue;
        }
        let tree = projection::tree(types, occurrence)?;
        let root = surface_node(types, occurrence)?;
        let Some(block) = syntax::descendant(tree, root, NodeKind::Block) else {
            continue;
        };
        if found.is_some() {
            return Err(HeaderDefinitionError::InvalidSurface(occurrence));
        }
        let body = types
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .program
            .declarations_mut()
            .add_body(Body::new(BodyOwner::Callable(callable)));
        projection::body(
            types,
            occurrence,
            body,
            projection::role(types, occurrence),
            block,
        )?;
        found = Some(body);
    }
    Ok(found)
}

fn allocate_body(
    types: &mut PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    owner: BodyOwner,
) -> Result<Option<BodyId>, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    let root = surface_node(types, declaration)?;
    let block = syntax::descendant(tree, root, NodeKind::Block)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?;
    let body = types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations_mut()
        .add_body(Body::new(owner));
    projection::body(types, declaration, body, SourceRole::Declaration, block)?;
    Ok(Some(body))
}

fn requirement_owner(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<RequirementOwner, HeaderDefinitionError> {
    match entity(types, declaration) {
        Some(ReservedEntity::NominalType(owner)) => Ok(RequirementOwner::NominalType(owner)),
        Some(ReservedEntity::TypeAlias(owner)) => Ok(RequirementOwner::TypeAlias(owner)),
        Some(ReservedEntity::Interface(owner)) => Ok(RequirementOwner::Interface(owner)),
        Some(ReservedEntity::AssociatedType(_)) => {
            let owner = surface_owner(types, declaration)?;
            match entity(types, owner) {
                Some(ReservedEntity::Interface(owner)) => Ok(RequirementOwner::Interface(owner)),
                _ => Err(HeaderDefinitionError::InvalidOwner(declaration)),
            }
        }
        Some(ReservedEntity::Callable(owner)) => Ok(RequirementOwner::Callable(owner)),
        Some(ReservedEntity::Instance(owner)) => Ok(RequirementOwner::Instance(owner)),
        Some(_) | None => Err(HeaderDefinitionError::InvalidOwner(declaration)),
    }
}

fn drop_receiver_token(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SyntaxToken, HeaderDefinitionError> {
    let tree = projection::tree(types, declaration)?;
    syntax::direct_tokens(tree, surface_node(types, declaration)?)
        .into_iter()
        .rev()
        .find(|token| token.kind() == TokenKind::Identifier)
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))
}

pub(super) fn pattern_type(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
    position: usize,
) -> Result<TypeId, HeaderDefinitionError> {
    match types
        .patterns
        .get(declaration.index())
        .and_then(|patterns| patterns.get(position))
    {
        Some(crate::NormalizedDeclarationPattern::Type(ty)) => Ok(*ty),
        _ => Err(HeaderDefinitionError::InvalidTypePattern(declaration)),
    }
}

pub(super) fn normalized_type(
    types: &PreparedTypes<'_>,
    node: NodeId,
) -> Result<TypeId, HeaderDefinitionError> {
    types
        .roots
        .get(&node)
        .copied()
        .ok_or(HeaderDefinitionError::MissingType(node))
}

pub(super) fn entity(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Option<ReservedEntity> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .entity(declaration)
}

pub(super) fn representative(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> SurfaceDeclarationId {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .contracts
        .representative(declaration)
}

pub(super) fn surface_count(types: &PreparedTypes<'_>) -> usize {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .len()
}

pub(super) fn surface_node(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<NodeId, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .map(|surface| surface.node())
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))
}

pub(super) fn surface_kind(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SurfaceDeclarationKind, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .map(|surface| surface.kind())
        .ok_or(HeaderDefinitionError::InvalidSurface(declaration))
}

pub(super) fn surface_owner(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<SurfaceDeclarationId, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .declarations
        .get(declaration.index())
        .and_then(|surface| surface.owner())
        .ok_or(HeaderDefinitionError::InvalidOwner(declaration))
}

pub(super) fn name(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::Symbol, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .name(declaration)
        .ok_or(HeaderDefinitionError::MissingName(declaration))
}

pub(super) fn site(
    types: &PreparedTypes<'_>,
    declaration: SurfaceDeclarationId,
) -> Result<nocter_model::DeclarationSiteId, HeaderDefinitionError> {
    types
        .namespaces
        .imports
        .generics
        .headers
        .site(declaration)
        .ok_or(HeaderDefinitionError::MissingSite(declaration))
}
