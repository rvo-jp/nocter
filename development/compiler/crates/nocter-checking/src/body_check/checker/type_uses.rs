use nocter_declarations::{BodyOwner, CallableOwner, ExportedEntity};
use nocter_model::{
    BorrowCapability, BuiltinType, CallableCapability, CallableContract, GenericParameterId,
    NominalTypeId, ParameterOrigin, ResultProvenance, Symbol, TypeId, TypeKind,
};
use nocter_source_index::{SemanticEntity, SourceOrigin};
use nocter_syntax::{NodeId, NodeKind, Punctuation, SyntaxElement, SyntaxToken, TokenKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{
    direct_child, direct_children, direct_identifier, direct_nodes, direct_token, identifier_tokens,
};
use crate::type_relations::TypeSubstitution;
use crate::{NameTarget, TypePosition, TypeValidityFailure, validate_type};

pub(super) struct ExplicitConstructionOwner {
    pub(super) definition: NominalTypeId,
    pub(super) arguments: Box<[TypeId]>,
    pub(super) member: SyntaxToken,
}

pub(super) struct InferredConstructionOwner {
    pub(super) reference: NodeId,
    pub(super) target: NameTarget,
}

pub(super) enum NominalOwnerArguments {
    Inferred(Box<[GenericParameterId]>),
    Fixed(Box<[TypeId]>),
}

pub(super) struct NominalConstructionOwner {
    pub(super) definition: NominalTypeId,
    pub(super) arguments: NominalOwnerArguments,
}

struct NamedSegment {
    token: SyntaxToken,
    arguments: Vec<NodeId>,
}

impl BodyChecker<'_, '_> {
    /// Resolves one authored body annotation and validates it as stored value data.
    ///
    /// Name binding, normalized type construction, generic requirements, and data-position shape
    /// remain separate rules, but every local and closure-parameter annotation enters through this
    /// boundary.
    pub(super) fn resolve_data_type_use(&mut self, node: NodeId) -> Result<TypeId, BodyCheckError> {
        self.resolve_type_use_in_position(node, TypePosition::Data)
    }

    pub(super) fn resolve_callable_result_type_use(
        &mut self,
        node: NodeId,
    ) -> Result<TypeId, BodyCheckError> {
        self.resolve_type_use_in_position(node, TypePosition::CallableResult)
    }

    fn resolve_type_use_in_position(
        &mut self,
        node: NodeId,
        position: TypePosition,
    ) -> Result<TypeId, BodyCheckError> {
        let ty = self.resolve_type_use(node)?;
        match validate_type(self.types, ty, position) {
            Ok(()) => Ok(ty),
            Err(TypeValidityFailure::Rule(violation)) => {
                let origin = SourceOrigin::from_node(self.tree(), node)
                    .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
                let rule = violation.rule();
                Err(BodyCheckError::from_type_validity(
                    rule,
                    rule.diagnostic(origin),
                ))
            }
            Err(TypeValidityFailure::UnknownType(unknown)) => {
                Err(BodyCheckInternalError::UnknownType(unknown).into())
            }
        }
    }

    pub(super) fn inferred_nominal_construction_type(
        &self,
        definition: NominalTypeId,
    ) -> Result<NominalConstructionOwner, BodyCheckInternalError> {
        let declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        Ok(NominalConstructionOwner {
            definition,
            arguments: NominalOwnerArguments::Inferred(
                declaration.generic_parameters().to_vec().into_boxed_slice(),
            ),
        })
    }

    /// Resolves the nominal owner of a structural or variant construction without prematurely
    /// rejecting omitted generic arguments.
    ///
    /// Ordinary type-use resolution requires complete type identity. Construction differs: a
    /// bare generic owner contributes its generic parameters to inference, while explicit owner
    /// arguments and lexical `Self` are already fixed types.
    pub(super) fn resolve_nominal_construction_type(
        &mut self,
        node: NodeId,
    ) -> Result<NominalConstructionOwner, BodyCheckError> {
        let segments = self.named_segments(node)?;
        let fixed = segments.iter().any(|segment| !segment.arguments.is_empty())
            || (segments.len() == 1 && self.token_text(segments[0].token)? == "Self");
        if fixed {
            let ty = self.resolve_named_segments(node, segments)?;
            let Some(TypeKind::Nominal {
                definition,
                arguments,
            }) = self.types.get(ty)
            else {
                return Err(self.rule(BodyRule::InvalidCall, node)?);
            };
            return Ok(NominalConstructionOwner {
                definition: *definition,
                arguments: NominalOwnerArguments::Fixed(arguments.clone()),
            });
        }

        let definition = self.resolve_unapplied_nominal(node, &segments)?;
        let declaration = self
            .graph
            .declarations()
            .nominal_types()
            .get(definition)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        Ok(NominalConstructionOwner {
            definition,
            arguments: NominalOwnerArguments::Inferred(
                declaration.generic_parameters().to_vec().into_boxed_slice(),
            ),
        })
    }

    pub(super) fn resolve_inferred_construction_owner(
        &mut self,
        node: NodeId,
    ) -> Result<InferredConstructionOwner, BodyCheckError> {
        let mut current = node;
        let mut selections = Vec::new();
        loop {
            while self
                .kind(current)
                .is_ok_and(crate::syntax::is_transparent_expression)
            {
                let children = direct_nodes(self.tree(), current);
                let [child] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(current).into());
                };
                current = *child;
            }
            if self.kind(current)? != NodeKind::PostfixExpression {
                break;
            }
            let children = direct_nodes(self.tree(), current);
            let [base, suffix] = children.as_slice() else {
                return Err(BodyCheckInternalError::InvalidSyntax(current).into());
            };
            if self.kind(*suffix)? != NodeKind::MemberSuffix {
                return Err(self.rule(BodyRule::InvalidCall, node)?);
            }
            selections.push(*suffix);
            current = *base;
        }
        if self.kind(current)? != NodeKind::ReferenceExpression {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        selections.reverse();
        let reference = current;
        let mut target = super::calls::call_name_target(self, reference)?;
        for suffix in selections {
            let token = direct_identifier(self.tree(), suffix)
                .ok_or(BodyCheckInternalError::InvalidSyntax(suffix))?;
            let name = self.segment_symbol(token)?;
            let NameTarget::Exported(ExportedEntity::Module(module)) = target else {
                return Err(self.rule(BodyRule::InvalidCall, suffix)?);
            };
            let Some(selected) = self.graph.lookup_export(self.source.module(), module, name)
            else {
                return Err(self.rule(BodyRule::InvalidCall, suffix)?);
            };
            self.project_exported(token, selected)?;
            target = NameTarget::Exported(selected);
        }
        Ok(InferredConstructionOwner { reference, target })
    }

    pub(super) fn resolve_explicit_construction_owner(
        &mut self,
        node: NodeId,
    ) -> Result<ExplicitConstructionOwner, BodyCheckError> {
        let named = direct_child(self.tree(), node, NodeKind::NamedType)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let mut segments = self.named_segments(named)?;
        let Some(member) = segments.pop() else {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        };
        if !member.arguments.is_empty() || segments.is_empty() {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        }
        let owner = self.resolve_named_segments(node, segments)?;
        let Some(TypeKind::Nominal {
            definition,
            arguments,
        }) = self.types.get(owner)
        else {
            return Err(self.rule(BodyRule::InvalidCall, node)?);
        };
        Ok(ExplicitConstructionOwner {
            definition: *definition,
            arguments: arguments.clone(),
            member: member.token,
        })
    }

    pub(super) fn resolve_type_use(&mut self, node: NodeId) -> Result<TypeId, BodyCheckError> {
        match self.kind(node)? {
            NodeKind::Type => {
                let children = direct_nodes(self.tree(), node);
                let [base] = children.as_slice() else {
                    return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                };
                let mut ty = self.resolve_type_use(*base)?;
                for element in self.tree().children(node) {
                    let SyntaxElement::Token(token) = element else {
                        continue;
                    };
                    ty = match token.kind() {
                        TokenKind::Punctuation(Punctuation::Question) => self
                            .types
                            .intern(TypeKind::Optional(ty))
                            .map_err(|_| BodyCheckInternalError::UnknownType(ty))?,
                        TokenKind::Punctuation(Punctuation::Bang) => self
                            .types
                            .intern(TypeKind::Fallible(ty))
                            .map_err(|_| BodyCheckInternalError::UnknownType(ty))?,
                        _ => ty,
                    };
                }
                Ok(ty)
            }
            NodeKind::BuiltinType => {
                let token = direct_token(self.tree(), node)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                let Some(builtin) = builtin_type(self.token_text(token)?) else {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                };
                Ok(self.types.builtin(builtin))
            }
            NodeKind::NamedType => {
                let segments = self.named_segments(node)?;
                self.resolve_named_segments(node, segments)
            }
            NodeKind::PointerType
            | NodeKind::BorrowType
            | NodeKind::SliceType
            | NodeKind::FixedArrayType
            | NodeKind::GroupedType => self.resolve_type_wrapper(node),
            NodeKind::CallableType => self.resolve_callable_type(node),
            _ => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
        }
    }

    fn resolve_type_wrapper(&mut self, node: NodeId) -> Result<TypeId, BodyCheckError> {
        let children = direct_nodes(self.tree(), node);
        let [child] = children.as_slice() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        let inner = self.resolve_type_use(*child)?;
        let kind = match self.kind(node)? {
            NodeKind::PointerType => TypeKind::Pointer(inner),
            NodeKind::BorrowType => TypeKind::Borrow {
                capability: if self.tree().children(node).iter().any(|element| {
                    matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Punctuation(Punctuation::ReadWrite))
                }) {
                    BorrowCapability::ReadWrite
                } else {
                    BorrowCapability::Readonly
                },
                referent: inner,
            },
            NodeKind::SliceType => TypeKind::Slice(inner),
            NodeKind::FixedArrayType => {
                let length = self
                    .tree()
                    .children(node)
                    .iter()
                    .find_map(|element| match element {
                        SyntaxElement::Token(token)
                            if token.kind() == TokenKind::IntegerLiteral =>
                        {
                            Some(*token)
                        }
                        _ => None,
                    })
                    .and_then(|token| self.token_text(token).ok())
                    .and_then(super::super::literal::parse_integer);
                let Some(length) = length else {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                };
                TypeKind::FixedArray {
                    element: inner,
                    length,
                }
            }
            NodeKind::GroupedType => return Ok(inner),
            _ => return Err(BodyCheckInternalError::InvalidSyntax(node).into()),
        };
        self.types
            .intern(kind)
            .map_err(|_| BodyCheckInternalError::UnknownType(inner).into())
    }

    fn resolve_callable_type(&mut self, node: NodeId) -> Result<TypeId, BodyCheckError> {
        let parameters_node = direct_child(self.tree(), node, NodeKind::CallableParameters)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let mut parameters = Vec::new();
        let mut names = Vec::new();
        for parameter in direct_children(self.tree(), parameters_node, NodeKind::CallableParameter)
        {
            let ty = direct_child(self.tree(), parameter, NodeKind::Type)
                .ok_or(BodyCheckInternalError::InvalidSyntax(parameter))?;
            parameters.push(self.resolve_type_use(ty)?);
            names.push(direct_identifier(self.tree(), parameter));
        }
        let result = direct_child(self.tree(), node, NodeKind::Type)
            .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
        let result = self.resolve_type_use(result)?;
        let provenance = if let Some(clause) =
            direct_child(self.tree(), node, NodeKind::ProvenanceClause)
        {
            let mut origins = Vec::new();
            let tokens = identifier_tokens(self.tree(), clause);
            for token in tokens.into_iter().skip(1) {
                let name = self.token_text(token)?;
                let Some(position) = names.iter().position(|candidate| {
                    candidate.is_some_and(|candidate| self.token_text(candidate).ok() == Some(name))
                }) else {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, clause)?);
                };
                origins.push(ParameterOrigin::new(position));
            }
            match ResultProvenance::from_origins(origins) {
                Ok(provenance) => provenance,
                Err(_) => return Err(self.rule(BodyRule::InvalidBodyTypeUse, clause)?),
            }
        } else {
            self.infer_callable_type_provenance(node, &parameters, &names, result)?
        };
        let capability = if self.tree().children(node).iter().any(|element| {
            matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Punctuation(Punctuation::ReadWrite))
        }) {
            CallableCapability::ReadWrite
        } else if self.tree().children(node).iter().any(|element| {
            matches!(element, SyntaxElement::Token(token) if token.kind() == TokenKind::Punctuation(Punctuation::Ampersand))
        }) {
            CallableCapability::Readonly
        } else {
            CallableCapability::Owned
        };
        let contract = CallableContract::new(capability, parameters, result, provenance)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?;
        self.types
            .intern(TypeKind::Callable(contract))
            .map_err(|_| BodyCheckInternalError::UnknownType(result).into())
    }

    fn infer_callable_type_provenance(
        &self,
        node: NodeId,
        parameters: &[TypeId],
        names: &[Option<SyntaxToken>],
        result: TypeId,
    ) -> Result<ResultProvenance, BodyCheckError> {
        if !self.types.may_carry_storage(result) {
            return Ok(ResultProvenance::empty());
        }
        let eligible = parameters
            .iter()
            .enumerate()
            .filter(|(position, ty)| {
                names[*position].is_some() && self.types.may_carry_storage(**ty)
            })
            .map(|(position, _)| ParameterOrigin::new(position))
            .collect::<Vec<_>>();
        let unnamed = parameters
            .iter()
            .enumerate()
            .any(|(position, ty)| names[position].is_none() && self.types.may_carry_storage(*ty));
        match (eligible.as_slice(), unnamed) {
            ([], false) => Ok(ResultProvenance::empty()),
            ([origin], false) => match ResultProvenance::from_origins([*origin]) {
                Ok(provenance) => Ok(provenance),
                Err(_) => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
            },
            _ => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
        }
    }

    fn named_segments(&self, node: NodeId) -> Result<Vec<NamedSegment>, BodyCheckError> {
        let mut segments = Vec::<NamedSegment>::new();
        for element in self.tree().children(node) {
            match element {
                SyntaxElement::Token(token) if token.kind() == TokenKind::Identifier => {
                    segments.push(NamedSegment {
                        token: *token,
                        arguments: Vec::new(),
                    });
                }
                SyntaxElement::Node(child)
                    if self
                        .kind(*child)
                        .is_ok_and(|kind| kind == NodeKind::SelfType) =>
                {
                    let token = direct_token(self.tree(), *child)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(*child))?;
                    segments.push(NamedSegment {
                        token,
                        arguments: Vec::new(),
                    });
                }
                SyntaxElement::Node(child)
                    if self
                        .kind(*child)
                        .is_ok_and(|kind| kind == NodeKind::TypeArguments) =>
                {
                    let Some(segment) = segments.last_mut() else {
                        return Err(BodyCheckInternalError::InvalidSyntax(node).into());
                    };
                    segment.arguments = direct_nodes(self.tree(), *child)
                        .into_iter()
                        .filter(|argument| {
                            self.kind(*argument)
                                .is_ok_and(|kind| kind == NodeKind::Type)
                        })
                        .collect();
                }
                _ => {}
            }
        }
        if segments.is_empty() {
            Err(BodyCheckInternalError::InvalidSyntax(node).into())
        } else {
            Ok(segments)
        }
    }

    fn resolve_named_segments(
        &mut self,
        node: NodeId,
        mut segments: Vec<NamedSegment>,
    ) -> Result<TypeId, BodyCheckError> {
        if segments[0].arguments.is_empty() {
            let name = self.segment_symbol(segments[0].token)?;
            let base = if let Some(parameter) = self.lexical_generic(name)? {
                self.project_type_entity(
                    segments[0].token,
                    SemanticEntity::GenericParameter(parameter),
                )?;
                Some(
                    self.types
                        .intern(TypeKind::GenericParameter(parameter))
                        .map_err(|_| BodyCheckInternalError::InvalidSyntax(node))?,
                )
            } else if self.token_text(segments[0].token)? == "Self" {
                Some(self.lexical_self_type(node, segments[0].token)?)
            } else {
                None
            };
            if let Some(base) = base {
                return match segments.as_slice() {
                    [_] => Ok(base),
                    [_, associated] if associated.arguments.is_empty() => {
                        self.resolve_associated_projection(node, base, associated.token)
                    }
                    _ => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
                };
            }
        }

        let first = segments.remove(0);
        let first_name = self.segment_symbol(first.token)?;
        let Some(mut entity) = self.graph.lookup_local(self.source.module(), first_name) else {
            return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
        };
        self.project_exported(first.token, entity)?;
        if !first.arguments.is_empty() {
            if !segments.is_empty() {
                return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
            }
            return self.resolve_type_entity(node, entity, first.arguments);
        }
        while matches!(entity, ExportedEntity::Module(_)) {
            let ExportedEntity::Module(module) = entity else {
                unreachable!()
            };
            let Some(segment) = (!segments.is_empty()).then(|| segments.remove(0)) else {
                return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
            };
            let name = self.segment_symbol(segment.token)?;
            let selected = self.graph.lookup_export(self.source.module(), module, name);
            let Some(selected) = selected else {
                return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
            };
            entity = selected;
            self.project_exported(segment.token, entity)?;
            if !segment.arguments.is_empty() {
                if !segments.is_empty() {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                }
                return self.resolve_type_entity(node, entity, segment.arguments);
            }
        }
        if !segments.is_empty() {
            return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
        }
        self.resolve_type_entity(node, entity, Vec::new())
    }

    fn resolve_associated_projection(
        &mut self,
        node: NodeId,
        base: TypeId,
        token: SyntaxToken,
    ) -> Result<TypeId, BodyCheckError> {
        let name = self.segment_symbol(token)?;
        let declarations = self.graph.declarations();
        let mut candidates = self
            .assumptions
            .iter()
            .map(crate::CheckedRequirement::predicate)
            .chain(self.intrinsic_facts.iter())
            .filter_map(|predicate| {
                let crate::CheckedPredicate::Capability {
                    subject,
                    capability: nocter_declarations::StructuralCapability::Interface(application),
                } = predicate
                else {
                    return None;
                };
                (*subject == base).then_some(application.interface())
            })
            .flat_map(|interface| {
                declarations
                    .interfaces()
                    .get(interface)
                    .map(nocter_declarations::InterfaceDeclaration::associated_types)
                    .unwrap_or_default()
                    .iter()
                    .copied()
            })
            .filter(|associated| {
                declarations
                    .associated_types()
                    .get(*associated)
                    .is_some_and(|declaration| declaration.name() == name)
            })
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();
        let [associated] = candidates.as_slice() else {
            return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
        };
        self.project_type_entity(token, SemanticEntity::AssociatedType(*associated))?;
        self.types
            .intern(TypeKind::AssociatedProjection {
                base,
                associated: *associated,
            })
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(node).into())
    }

    fn resolve_unapplied_nominal(
        &mut self,
        node: NodeId,
        segments: &[NamedSegment],
    ) -> Result<NominalTypeId, BodyCheckError> {
        let Some(first) = segments.first() else {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        };
        if segments.iter().any(|segment| !segment.arguments.is_empty()) {
            return Err(BodyCheckInternalError::InvalidSyntax(node).into());
        }
        let first_name = self.segment_symbol(first.token)?;
        let Some(mut entity) = self.graph.lookup_local(self.source.module(), first_name) else {
            return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
        };
        self.project_exported(first.token, entity)?;
        for segment in &segments[1..] {
            let ExportedEntity::Module(module) = entity else {
                return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
            };
            let name = self.segment_symbol(segment.token)?;
            let Some(selected) = self.graph.lookup_export(self.source.module(), module, name)
            else {
                return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
            };
            entity = selected;
            self.project_exported(segment.token, entity)?;
        }
        match entity {
            ExportedEntity::NominalType(definition) => Ok(definition),
            _ => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
        }
    }

    fn resolve_type_entity(
        &mut self,
        node: NodeId,
        entity: ExportedEntity,
        arguments: Vec<NodeId>,
    ) -> Result<TypeId, BodyCheckError> {
        let arguments = arguments
            .into_iter()
            .map(|argument| self.resolve_type_use(argument))
            .collect::<Result<Vec<_>, _>>()?;
        match entity {
            ExportedEntity::NominalType(definition) => {
                let nominal = self
                    .graph
                    .declarations()
                    .nominal_types()
                    .get(definition)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                if nominal.generic_parameters().len() != arguments.len() {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                }
                let parameters = nominal.generic_parameters().to_vec();
                let requirements = nominal.requirements().to_vec();
                let mut substitution = TypeSubstitution::default();
                for (parameter, argument) in
                    parameters.iter().copied().zip(arguments.iter().copied())
                {
                    substitution.bind_generic(parameter, argument);
                }
                if !self.requirements_hold(&requirements, &substitution)? {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                }
                self.types
                    .intern(TypeKind::Nominal {
                        definition,
                        arguments: arguments.into_boxed_slice(),
                    })
                    .map_err(|_| BodyCheckInternalError::InvalidSyntax(node).into())
            }
            ExportedEntity::TypeAlias(alias) => {
                let alias = self
                    .graph
                    .declarations()
                    .type_aliases()
                    .get(alias)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                if alias.generic_parameters().len() != arguments.len() {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                }
                let parameters = alias.generic_parameters().to_vec();
                let requirements = alias.requirements().to_vec();
                let target = alias.target();
                let mut substitution = TypeSubstitution::default();
                for (parameter, argument) in parameters.iter().copied().zip(arguments) {
                    substitution.bind_generic(parameter, argument);
                }
                if !self.requirements_hold(&requirements, &substitution)? {
                    return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
                }
                substitution
                    .apply_type(self.types, target)
                    .map_err(BodyCheckInternalError::CallSubstitution)
                    .map_err(Into::into)
            }
            _ => Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?),
        }
    }

    fn lexical_generic(
        &self,
        name: Symbol,
    ) -> Result<Option<GenericParameterId>, BodyCheckInternalError> {
        let declarations = self.graph.declarations();
        for parameter in self.lexical_generic_parameters()? {
            let declaration = declarations
                .generic_parameters()
                .get(parameter)
                .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
            if declaration.name() == name {
                return Ok(Some(parameter));
            }
        }
        Ok(None)
    }

    fn lexical_generic_parameters(
        &self,
    ) -> Result<Vec<GenericParameterId>, BodyCheckInternalError> {
        let declarations = self.graph.declarations();
        match self.source.owner() {
            BodyOwner::Callable(callable) => {
                let callable = declarations
                    .callables()
                    .get(callable)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
                let owner = match callable.owner() {
                    CallableOwner::Module(_) => &[][..],
                    CallableOwner::Construction(owner) => declarations
                        .constructions()
                        .get(owner)
                        .map(nocter_declarations::ConstructionDeclaration::generic_parameters)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?,
                    CallableOwner::Instance(owner) => declarations
                        .instances()
                        .get(owner)
                        .map(nocter_declarations::InstanceDeclaration::generic_parameters)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?,
                    CallableOwner::Interface(owner) => declarations
                        .interfaces()
                        .get(owner)
                        .map(nocter_declarations::InterfaceDeclaration::generic_parameters)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?,
                    CallableOwner::Conformance(owner) => declarations
                        .conformances()
                        .get(owner)
                        .map(nocter_declarations::ConformanceDeclaration::generic_parameters)
                        .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))?,
                };
                Ok(owner
                    .iter()
                    .chain(callable.generic_parameters())
                    .copied()
                    .collect())
            }
            BodyOwner::Drop(drop) => declarations
                .drops()
                .get(drop)
                .map(|drop| drop.generic_parameters().to_vec())
                .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block())),
            BodyOwner::Test(_) => Ok(Vec::new()),
        }
    }

    fn lexical_self_type(
        &mut self,
        node: NodeId,
        token: SyntaxToken,
    ) -> Result<TypeId, BodyCheckError> {
        let declarations = self.graph.declarations();
        let (ty, entity) = match self.source.owner() {
            BodyOwner::Callable(callable) => {
                let callable = declarations
                    .callables()
                    .get(callable)
                    .ok_or(BodyCheckInternalError::InvalidSyntax(node))?;
                match callable.owner() {
                    CallableOwner::Construction(owner) => (
                        declarations
                            .constructions()
                            .get(owner)
                            .map(nocter_declarations::ConstructionDeclaration::target),
                        Some(SemanticEntity::Construction(owner)),
                    ),
                    CallableOwner::Instance(owner) => (
                        declarations
                            .instances()
                            .get(owner)
                            .map(nocter_declarations::InstanceDeclaration::target),
                        Some(SemanticEntity::Instance(owner)),
                    ),
                    CallableOwner::Conformance(owner) => (
                        declarations
                            .conformances()
                            .get(owner)
                            .map(nocter_declarations::ConformanceDeclaration::target),
                        Some(SemanticEntity::Conformance(owner)),
                    ),
                    CallableOwner::Interface(owner) => (
                        self.types.intern(TypeKind::InterfaceSelf(owner)).ok(),
                        Some(SemanticEntity::Interface(owner)),
                    ),
                    CallableOwner::Module(_) => (None, None),
                }
            }
            BodyOwner::Drop(drop) => (
                declarations
                    .drops()
                    .get(drop)
                    .map(nocter_declarations::DropDeclaration::target),
                Some(SemanticEntity::Drop(drop)),
            ),
            BodyOwner::Test(_) => (None, None),
        };
        let Some(ty) = ty else {
            return Err(self.rule(BodyRule::InvalidBodyTypeUse, node)?);
        };
        if let Some(entity) = entity {
            self.project_type_entity(token, entity)?;
        }
        Ok(ty)
    }

    pub(super) fn segment_symbol(
        &self,
        token: SyntaxToken,
    ) -> Result<Symbol, BodyCheckInternalError> {
        self.graph
            .symbols()
            .get(self.token_text(token)?)
            .ok_or(BodyCheckInternalError::InvalidSyntax(self.source.block()))
    }

    pub(super) fn project_exported(
        &mut self,
        token: SyntaxToken,
        entity: ExportedEntity,
    ) -> Result<(), BodyCheckInternalError> {
        let entity = match entity {
            ExportedEntity::Module(id) => SemanticEntity::Module(id),
            ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
            ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
            ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
            ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
        };
        self.project_type_entity(token, entity)
    }

    pub(super) fn project_type_entity(
        &mut self,
        token: SyntaxToken,
        entity: SemanticEntity,
    ) -> Result<(), BodyCheckInternalError> {
        let origin = SourceOrigin::from_token(self.tree(), token)
            .map_err(|_| BodyCheckInternalError::InvalidSyntax(self.source.block()))?;
        self.projections
            .push(super::NodeProjection::new(entity, origin));
        Ok(())
    }
}

fn builtin_type(spelling: &str) -> Option<BuiltinType> {
    match spelling {
        "bool" => Some(BuiltinType::Bool),
        "i8" => Some(BuiltinType::I8),
        "i16" => Some(BuiltinType::I16),
        "i32" => Some(BuiltinType::I32),
        "i64" => Some(BuiltinType::I64),
        "u8" => Some(BuiltinType::U8),
        "u16" => Some(BuiltinType::U16),
        "u32" => Some(BuiltinType::U32),
        "u64" => Some(BuiltinType::U64),
        "usize" => Some(BuiltinType::Usize),
        "isize" => Some(BuiltinType::Isize),
        "str" => Some(BuiltinType::Str),
        "error" => Some(BuiltinType::Error),
        "void" => Some(BuiltinType::Void),
        "never" => Some(BuiltinType::Never),
        _ => None,
    }
}
