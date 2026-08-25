use std::collections::{HashMap, HashSet};

use nocter_constant_evaluation::{
    ConstantEvaluationError, ConstantEvaluationRule, ConstantExpressionPlan, ConstantPlanError,
    ConstantPlanRule, ConstantReference, ConstantResolver, ConstantScalarType,
    evaluate_constant_plans, evaluate_expression_plan, plan_expression,
};
use nocter_declarations::ExportedEntity;
use nocter_model::{BorrowCapability, BuiltinType, ConstantId, ConstantValue, ModuleId};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{NodeId, NodeKind, SyntaxElement, SyntaxToken, SyntaxTree, TokenKind};

use crate::definitions::HeaderDefinitionError;
use crate::{
    DefinitionRule, DefinitionViolation, ReservedEntity, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

use super::super::{BoundTypeId, BoundTypeKind, PreparedConstantValue, PreparedTypeBindings};

#[derive(Clone, Copy)]
struct ConstantSource {
    declaration: SurfaceDeclarationId,
    initializer: NodeId,
    ty: BoundTypeId,
}

struct HeaderResolver<'a, 'syntax> {
    bindings: &'a PreparedTypeBindings<'syntax>,
    sources: &'a HashMap<ConstantId, ConstantSource>,
    source_ids: &'a HashMap<SourceId, crate::SurfaceSourceId>,
    references: HashMap<NodeId, ConstantReference>,
    reference_projections: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
}

/// Plans and evaluates every header constant before structural type normalization.
///
/// The shared constant-evaluation crate owns expression typing, arithmetic, short-circuiting, and
/// dependency cycles. This adapter owns only declaration namespaces, bound header types, and
/// source projection.
///
/// # Errors
///
/// Returns a source-backed declaration rule or an internal header-contract failure when a bound
/// name, type, source, or projection is inconsistent.
pub fn evaluate(
    mut bindings: PreparedTypeBindings<'_>,
) -> Result<PreparedTypeBindings<'_>, HeaderDefinitionError> {
    let sources = collect_sources(&bindings)?;
    let source_ids = bindings
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .sources
        .iter()
        .enumerate()
        .map(|(index, source)| {
            (
                source.syntax().source(),
                crate::SurfaceSourceId::from_index(index),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut resolver = HeaderResolver {
        bindings: &bindings,
        sources: &sources,
        source_ids: &source_ids,
        references: HashMap::new(),
        reference_projections: HashMap::new(),
    };

    let mut plans = HashMap::<ConstantId, ConstantExpressionPlan>::new();
    let mut ids = sources.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    for id in ids {
        let source = sources[&id];
        let expected = resolver
            .scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                rule_at(
                    DefinitionRule::InvalidConstantType,
                    SyntaxOrigin::Node(source.initializer),
                )
            })?;
        let (file, tree) = syntax_input(&bindings, &source_ids, source.initializer)?;
        let plan = plan_expression(file, tree, source.initializer, expected, &mut resolver)
            .map_err(|error| plan_error(&resolver, error))?;
        plans.insert(id, plan);
    }
    let values =
        evaluate_constant_plans(&plans).map_err(|error| evaluation_error(&resolver, error))?;

    let usize_ty = ConstantScalarType::Integer(BuiltinType::Usize);
    let mut array_lengths = HashMap::new();
    for expression in collect_array_expressions(&bindings) {
        let (file, tree) = syntax_input(&bindings, &source_ids, expression)?;
        let plan = plan_expression(file, tree, expression, usize_ty, &mut resolver)
            .map_err(|error| plan_error(&resolver, error))?;
        let value = evaluate_expression_plan(&plan, |id| values.get(&id).cloned())
            .map_err(|error| evaluation_error(&resolver, error))?;
        let ConstantValue::Integer(value) = value else {
            return Err(rule_at(
                DefinitionRule::ConstantTypeMismatch,
                SyntaxOrigin::Node(expression),
            ));
        };
        let length = u64::try_from(value).map_err(|_| {
            rule_at(
                DefinitionRule::ConstantArithmeticFailure,
                SyntaxOrigin::Node(expression),
            )
        })?;
        array_lengths.insert(expression, length);
    }

    let constant_values = values
        .into_iter()
        .map(|(id, value)| {
            (
                id,
                PreparedConstantValue {
                    declaration: sources[&id].declaration,
                    value,
                },
            )
        })
        .collect();
    let projections = std::mem::take(&mut resolver.reference_projections);
    drop(resolver);
    project_references(&mut bindings, projections)?;
    bindings.constant_values = constant_values;
    bindings.array_lengths = array_lengths;
    Ok(bindings)
}

fn collect_array_expressions(bindings: &PreparedTypeBindings<'_>) -> Vec<NodeId> {
    let mut expressions = bindings
        .kinds
        .iter()
        .filter_map(|kind| match kind {
            BoundTypeKind::FixedArray { length, .. } => Some(*length),
            _ => None,
        })
        .collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|node| (node.source(), node.index()));
    expressions.dedup();
    expressions
}

fn collect_sources(
    bindings: &PreparedTypeBindings<'_>,
) -> Result<HashMap<ConstantId, ConstantSource>, HeaderDefinitionError> {
    let reserved = &bindings.namespaces.imports.generics.headers.reserved;
    let mut result = HashMap::new();
    for (index, entity) in reserved.entities.iter().copied().enumerate() {
        let Some(ReservedEntity::Constant(id)) = entity else {
            continue;
        };
        let declaration = SurfaceDeclarationId::from_index(index);
        let surface = reserved.declarations[index];
        if surface.kind() != SurfaceDeclarationKind::Constant {
            return Err(HeaderDefinitionError::InvalidSurface(declaration));
        }
        let tree = reserved
            .sources
            .get(surface.source().index())
            .ok_or(HeaderDefinitionError::InvalidSurface(declaration))?
            .syntax();
        let Some(initializer) = direct_node(tree, surface.node(), NodeKind::Expression) else {
            continue;
        };
        let representative = reserved.contracts.representative(declaration);
        let representative_surface = reserved.declarations[representative.index()];
        let representative_tree = reserved
            .sources
            .get(representative_surface.source().index())
            .ok_or(HeaderDefinitionError::InvalidSurface(representative))?
            .syntax();
        let ty_node = direct_node(
            representative_tree,
            representative_surface.node(),
            NodeKind::Type,
        )
        .ok_or(HeaderDefinitionError::MissingType(
            representative_surface.node(),
        ))?;
        let ty = bindings
            .roots
            .get(&ty_node)
            .copied()
            .ok_or(HeaderDefinitionError::MissingType(ty_node))?;
        if result
            .insert(
                id,
                ConstantSource {
                    declaration: representative,
                    initializer,
                    ty,
                },
            )
            .is_some()
        {
            return Err(HeaderDefinitionError::InvalidSurface(declaration));
        }
    }
    Ok(result)
}

impl ConstantResolver for HeaderResolver<'_, '_> {
    type Error = HeaderDefinitionError;

    fn resolve_constant(&mut self, node: NodeId) -> Result<ConstantReference, Self::Error> {
        if let Some(reference) = self.references.get(&node) {
            return Ok(*reference);
        }
        let mut projections = Vec::new();
        let ExportedEntity::Constant(id) = self.resolve_entity(node, &mut projections)? else {
            return Err(rule_at(
                DefinitionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            ));
        };
        let source = self.sources.get(&id).ok_or_else(|| self.internal(node))?;
        let ty = self
            .scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                rule_at(
                    DefinitionRule::InvalidConstantType,
                    SyntaxOrigin::Node(node),
                )
            })?;
        for (token, entity) in projections {
            let origin = SourceOrigin::from_token(self.tree(node)?, token)
                .map_err(|_| HeaderDefinitionError::InconsistentSource(token.source()))?;
            if self
                .reference_projections
                .insert(token, (entity, origin))
                .is_some_and(|(existing, _)| existing != entity)
            {
                return Err(self.internal(node));
            }
        }
        let reference = ConstantReference::new(id, ty);
        self.references.insert(node, reference);
        Ok(reference)
    }

    fn resolve_type(&mut self, node: NodeId) -> Result<Option<ConstantScalarType>, Self::Error> {
        let Some(bound) = self.bindings.roots.get(&node).copied() else {
            return Err(HeaderDefinitionError::MissingType(node));
        };
        Ok(self.scalar_type(bound, &mut HashSet::new()))
    }
}

impl HeaderResolver<'_, '_> {
    fn scalar_type(
        &self,
        ty: BoundTypeId,
        active_aliases: &mut HashSet<nocter_model::TypeAliasId>,
    ) -> Option<ConstantScalarType> {
        match self.bindings.kinds.get(ty.index())? {
            BoundTypeKind::Builtin(BuiltinType::Bool) => Some(ConstantScalarType::Bool),
            BoundTypeKind::Builtin(builtin) if integer_builtin(*builtin) => {
                Some(ConstantScalarType::Integer(*builtin))
            }
            BoundTypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            } if matches!(
                self.bindings.kinds.get(referent.index()),
                Some(BoundTypeKind::Builtin(BuiltinType::Str))
            ) =>
            {
                Some(ConstantScalarType::Text)
            }
            BoundTypeKind::Alias {
                definition,
                arguments,
            } if arguments.is_empty() && active_aliases.insert(*definition) => {
                let target = self.bindings.alias_targets.get(definition).copied()?;
                let result = self.scalar_type(target, active_aliases);
                active_aliases.remove(definition);
                result
            }
            _ => None,
        }
    }

    fn resolve_entity(
        &self,
        node: NodeId,
        projections: &mut Vec<(SyntaxToken, ExportedEntity)>,
    ) -> Result<ExportedEntity, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        match tree.node(node).map(nocter_syntax::SyntaxNode::kind) {
            Some(NodeKind::ReferenceExpression) => {
                let token = direct_token(tree, node).ok_or_else(|| self.internal(node))?;
                let symbol = self.symbol(token)?;
                let source = self.surface_source(node)?;
                let entity = self
                    .bindings
                    .namespaces
                    .lookup_local(source, symbol)
                    .ok_or_else(|| non_constant(node))?;
                projections.push((token, entity));
                Ok(entity)
            }
            Some(NodeKind::PostfixExpression) => {
                let nodes = direct_nodes(tree, node);
                if nodes.len() != 2
                    || tree.node(nodes[1]).map(nocter_syntax::SyntaxNode::kind)
                        != Some(NodeKind::MemberSuffix)
                {
                    return Err(non_constant(node));
                }
                let ExportedEntity::Module(module) = self.resolve_entity(nodes[0], projections)?
                else {
                    return Err(non_constant(node));
                };
                let token = direct_token(tree, nodes[1])
                    .filter(|token| token.kind() == TokenKind::Identifier)
                    .ok_or_else(|| self.internal(node))?;
                let entity = self
                    .bindings
                    .namespaces
                    .lookup_export(self.current_module(node)?, module, self.symbol(token)?)
                    .ok_or_else(|| non_constant(node))?;
                projections.push((token, entity));
                Ok(entity)
            }
            _ => Err(non_constant(node)),
        }
    }

    fn tree(&self, node: NodeId) -> Result<&SyntaxTree, HeaderDefinitionError> {
        let reserved = &self.bindings.namespaces.imports.generics.headers.reserved;
        self.source_ids
            .get(&node.source())
            .and_then(|source| reserved.sources.get(source.index()))
            .map(crate::SurfaceSource::syntax)
            .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))
    }

    fn surface_source(
        &self,
        node: NodeId,
    ) -> Result<crate::SurfaceSourceId, HeaderDefinitionError> {
        self.source_ids
            .get(&node.source())
            .copied()
            .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))
    }

    fn current_module(&self, node: NodeId) -> Result<ModuleId, HeaderDefinitionError> {
        self.bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .module_for_source(self.surface_source(node)?)
            .ok_or_else(|| self.internal(node))
    }

    fn symbol(&self, token: SyntaxToken) -> Result<nocter_model::Symbol, HeaderDefinitionError> {
        self.bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .symbols()
            .get(self.token_text(token)?)
            .ok_or_else(|| self.internal_token(token))
    }

    fn token_text(&self, token: SyntaxToken) -> Result<&str, HeaderDefinitionError> {
        self.bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .source_map
            .get(token.source())
            .and_then(|source| source.text_at(token.range()))
            .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))
    }

    fn internal(&self, node: NodeId) -> HeaderDefinitionError {
        self.declaration_for_source(node.source()).map_or(
            HeaderDefinitionError::InconsistentSource(node.source()),
            HeaderDefinitionError::InvalidSurface,
        )
    }

    fn internal_token(&self, token: SyntaxToken) -> HeaderDefinitionError {
        self.declaration_for_source(token.source()).map_or(
            HeaderDefinitionError::InconsistentSource(token.source()),
            HeaderDefinitionError::InvalidSurface,
        )
    }

    fn declaration_for_source(&self, source: SourceId) -> Option<SurfaceDeclarationId> {
        let reserved = &self.bindings.namespaces.imports.generics.headers.reserved;
        reserved
            .declarations
            .iter()
            .position(|declaration| {
                reserved.sources[declaration.source().index()]
                    .syntax()
                    .source()
                    == source
            })
            .map(SurfaceDeclarationId::from_index)
    }
}

fn syntax_input<'a>(
    bindings: &'a PreparedTypeBindings<'_>,
    source_ids: &HashMap<SourceId, crate::SurfaceSourceId>,
    node: NodeId,
) -> Result<(&'a nocter_source::SourceFile, &'a SyntaxTree), HeaderDefinitionError> {
    let reserved = &bindings.namespaces.imports.generics.headers.reserved;
    let file = reserved
        .source_map
        .get(node.source())
        .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))?;
    let tree = source_ids
        .get(&node.source())
        .and_then(|source| reserved.sources.get(source.index()))
        .map(crate::SurfaceSource::syntax)
        .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))?;
    Ok((file, tree))
}

fn plan_error(
    resolver: &HeaderResolver<'_, '_>,
    error: ConstantPlanError<HeaderDefinitionError>,
) -> HeaderDefinitionError {
    match error {
        ConstantPlanError::Context(error) => error,
        ConstantPlanError::Rule { rule, origin } => rule_at(
            match rule {
                ConstantPlanRule::NonConstantExpression => DefinitionRule::NonConstantExpression,
                ConstantPlanRule::TypeMismatch => DefinitionRule::ConstantTypeMismatch,
            },
            origin,
        ),
        ConstantPlanError::InvalidSyntax(node) => resolver.internal(node),
    }
}

fn evaluation_error(
    resolver: &HeaderResolver<'_, '_>,
    error: ConstantEvaluationError,
) -> HeaderDefinitionError {
    match error.rule() {
        ConstantEvaluationRule::ArithmeticFailure => {
            rule_at(DefinitionRule::ConstantArithmeticFailure, error.origin())
        }
        ConstantEvaluationRule::DependencyCycle => {
            rule_at(DefinitionRule::ConstantCycle, error.origin())
        }
        ConstantEvaluationRule::MissingConstant | ConstantEvaluationRule::InvalidPlan => {
            match error.origin() {
                SyntaxOrigin::Node(node) => resolver.internal(node),
                SyntaxOrigin::Token(token) => resolver.internal_token(token),
            }
        }
    }
}

fn project_references(
    bindings: &mut PreparedTypeBindings<'_>,
    references: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
) -> Result<(), HeaderDefinitionError> {
    let mut references = references.into_iter().collect::<Vec<_>>();
    references.sort_unstable_by_key(|(token, _)| {
        (token.source(), token.range().start(), token.range().end())
    });
    for (_, (entity, origin)) in references {
        bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .source_index
            .insert(semantic_entity(entity), SourceRole::Reference, origin)?;
    }
    Ok(())
}

const fn rule_at(rule: DefinitionRule, origin: SyntaxOrigin) -> HeaderDefinitionError {
    HeaderDefinitionError::Rule(DefinitionViolation::new(rule, origin))
}

const fn non_constant(node: NodeId) -> HeaderDefinitionError {
    rule_at(
        DefinitionRule::NonConstantExpression,
        SyntaxOrigin::Node(node),
    )
}

const fn integer_builtin(builtin: BuiltinType) -> bool {
    matches!(
        builtin,
        BuiltinType::I8
            | BuiltinType::I16
            | BuiltinType::I32
            | BuiltinType::I64
            | BuiltinType::Isize
            | BuiltinType::U8
            | BuiltinType::U16
            | BuiltinType::U32
            | BuiltinType::U64
            | BuiltinType::Usize
    )
}

fn direct_node(tree: &SyntaxTree, node: NodeId, kind: NodeKind) -> Option<NodeId> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Node(child)
                if tree.node(*child).is_some_and(|node| node.kind() == kind) =>
            {
                Some(*child)
            }
            _ => None,
        })
}

fn direct_nodes(tree: &SyntaxTree, node: NodeId) -> Vec<NodeId> {
    tree.children(node)
        .iter()
        .filter_map(|element| match element {
            SyntaxElement::Node(node) => Some(*node),
            _ => None,
        })
        .collect()
}

fn direct_token(tree: &SyntaxTree, node: NodeId) -> Option<SyntaxToken> {
    tree.children(node)
        .iter()
        .find_map(|element| match element {
            SyntaxElement::Token(token) => Some(*token),
            _ => None,
        })
}

const fn semantic_entity(entity: ExportedEntity) -> SemanticEntity {
    match entity {
        ExportedEntity::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
        ExportedEntity::Module(id) => SemanticEntity::Module(id),
        ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        ExportedEntity::Constant(id) => SemanticEntity::Constant(id),
        ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}
