use std::collections::{HashMap, HashSet};

use nocter_constant_evaluation::{
    ConstantEvaluationError, ConstantEvaluationRule, ConstantExpressionPlan, ConstantPlanError,
    ConstantPlanRule, ConstantReference, ConstantResolver, ConstantScalarType,
    DependencyComputation, DependencyQuery, DependencyQueryError, FloatFormat, FrozenType,
    evaluate_expression_plan, evaluate_frozen_expression_plan, plan_expression,
    plan_frozen_expression,
};
use nocter_declarations::ExportedEntity;
use nocter_model::{
    BorrowCapability, BuiltinType, CompilationTarget, ConstantExpressionId, ConstantId,
    ConstantValue, FrozenValue, ModuleId, StaticId,
};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole};
use nocter_syntax::{
    NodeId, NodeKind, SyntaxOrigin, SyntaxToken, SyntaxTree, TokenKind, child_nodes, direct_node,
    first_direct_token,
};

use crate::definitions::HeaderDefinitionError;
use crate::{
    DefinitionRule, DefinitionViolation, ReservedEntity, SurfaceDeclarationId,
    SurfaceDeclarationKind,
};

use super::super::{
    BoundTypeId, BoundTypeKind, PreparedConstantValue, PreparedStaticValue, PreparedTypeBindings,
};

#[derive(Clone, Copy)]
struct ConstantSource {
    declaration: SurfaceDeclarationId,
    initializer: NodeId,
    ty: BoundTypeId,
}

#[derive(Clone, Copy)]
struct StaticSource {
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

/// Plans and evaluates every header const and static before structural type normalization.
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
    let target = bindings
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .target();
    let sources = collect_sources(&bindings)?;
    let static_sources = collect_static_sources(&bindings)?;
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

    let evaluated = evaluate_header_values(
        target,
        &bindings,
        &source_ids,
        &sources,
        &static_sources,
        &mut resolver,
    )?;

    let constant_values = evaluated
        .constants
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
    project_references(&mut bindings, projections);
    bindings.constant_values = constant_values;
    bindings.static_values = evaluated.statics;
    bindings.array_lengths = evaluated.array_lengths;
    Ok(bindings)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HeaderValueKey {
    Constant(ConstantId),
    ArrayLength(ConstantExpressionId),
    Static(StaticId),
}

#[derive(Clone, Debug)]
enum HeaderValue {
    Constant(ConstantValue),
    ArrayLength(u64),
    Static(FrozenValue),
}

struct EvaluatedHeaderValues {
    constants: HashMap<ConstantId, ConstantValue>,
    array_lengths: HashMap<ConstantExpressionId, u64>,
    statics: HashMap<StaticId, PreparedStaticValue>,
}

struct HeaderValueComputation<'input, 'syntax, 'resolver> {
    target: CompilationTarget,
    bindings: &'input PreparedTypeBindings<'syntax>,
    source_ids: &'input HashMap<SourceId, crate::SurfaceSourceId>,
    constant_plans: HashMap<ConstantId, ConstantExpressionPlan>,
    array_length_plans: HashMap<ConstantExpressionId, ConstantExpressionPlan>,
    static_sources: &'input HashMap<StaticId, StaticSource>,
    resolver: &'resolver mut HeaderResolver<'input, 'syntax>,
}

fn evaluate_header_values<'input, 'syntax>(
    target: CompilationTarget,
    bindings: &'input PreparedTypeBindings<'syntax>,
    source_ids: &'input HashMap<SourceId, crate::SurfaceSourceId>,
    constant_sources: &HashMap<ConstantId, ConstantSource>,
    static_sources: &'input HashMap<StaticId, StaticSource>,
    resolver: &mut HeaderResolver<'input, 'syntax>,
) -> Result<EvaluatedHeaderValues, HeaderDefinitionError> {
    let mut constant_plans = HashMap::new();
    let mut constant_ids = constant_sources.keys().copied().collect::<Vec<_>>();
    constant_ids.sort_unstable();
    for id in &constant_ids {
        let source = constant_sources[id];
        let expected = resolver
            .scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                rule_at(
                    DefinitionRule::InvalidCompileTimeValueType,
                    SyntaxOrigin::Node(source.initializer),
                )
            })?;
        let (file, tree) = syntax_input(bindings, source_ids, source.initializer)?;
        let syntax = nocter_syntax::BoundSyntax::new(file, tree)
            .ok_or_else(|| inconsistent_node(source.initializer))?;
        let plan = plan_expression(target, syntax, source.initializer, expected, resolver)
            .map_err(plan_error)?;
        constant_plans.insert(*id, plan);
    }

    let usize_ty = ConstantScalarType::Integer(BuiltinType::Usize);
    let mut array_length_plans = HashMap::new();
    let mut array_length_ids = Vec::new();
    for (id, expression) in bindings.array_expressions.iter() {
        array_length_ids.push(id);
        let expression = *expression;
        let (file, tree) = syntax_input(bindings, source_ids, expression)?;
        let syntax = nocter_syntax::BoundSyntax::new(file, tree)
            .ok_or_else(|| inconsistent_node(expression))?;
        let plan =
            plan_expression(target, syntax, expression, usize_ty, resolver).map_err(plan_error)?;
        array_length_plans.insert(id, plan);
    }

    let mut static_ids = static_sources.keys().copied().collect::<Vec<_>>();
    static_ids.sort_unstable();

    let mut computation = HeaderValueComputation {
        target,
        bindings,
        source_ids,
        constant_plans,
        array_length_plans,
        static_sources,
        resolver,
    };
    let mut query = DependencyQuery::default();
    // Resolve aggregate roots first so correctness cannot depend on an eager constants-first pass.
    // Static type construction requests array lengths, and length plans request constants through
    // this same authority.
    for key in static_ids
        .iter()
        .copied()
        .map(HeaderValueKey::Static)
        .chain(
            array_length_ids
                .iter()
                .copied()
                .map(HeaderValueKey::ArrayLength),
        )
        .chain(constant_ids.iter().copied().map(HeaderValueKey::Constant))
    {
        if let Err(error) = query.resolve(&mut computation, key) {
            return Err(computation.query_error(key, error));
        }
    }

    let constants = constant_ids
        .into_iter()
        .map(|id| match query.completed(&HeaderValueKey::Constant(id)) {
            Some(HeaderValue::Constant(value)) => Ok((id, value.clone())),
            _ => Err(inconsistent_node(constant_sources[&id].initializer)),
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    let array_lengths = array_length_ids
        .into_iter()
        .map(
            |id| match query.completed(&HeaderValueKey::ArrayLength(id)) {
                Some(HeaderValue::ArrayLength(value)) => Ok((id, *value)),
                _ => Err(inconsistent_node(
                    *bindings
                        .array_expressions
                        .get(id)
                        .expect("queried array-length identity must retain its bound expression"),
                )),
            },
        )
        .collect::<Result<HashMap<_, _>, _>>()?;
    let statics = static_ids
        .into_iter()
        .map(|id| match query.completed(&HeaderValueKey::Static(id)) {
            Some(HeaderValue::Static(value)) => Ok((
                id,
                PreparedStaticValue {
                    declaration: static_sources[&id].declaration,
                    value: value.clone(),
                },
            )),
            _ => Err(inconsistent_node(static_sources[&id].initializer)),
        })
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(EvaluatedHeaderValues {
        constants,
        array_lengths,
        statics,
    })
}

type HeaderQueryError = DependencyQueryError<HeaderValueKey, HeaderDefinitionError>;

impl DependencyComputation<HeaderValueKey, HeaderValue, HeaderDefinitionError>
    for HeaderValueComputation<'_, '_, '_>
{
    fn compute(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        key: HeaderValueKey,
    ) -> Result<HeaderValue, HeaderQueryError> {
        match key {
            HeaderValueKey::Constant(id) => self.constant(query, id),
            HeaderValueKey::ArrayLength(id) => self.array_length(query, id),
            HeaderValueKey::Static(id) => self.static_value(query, id),
        }
    }
}

impl HeaderValueComputation<'_, '_, '_> {
    fn constant(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        id: ConstantId,
    ) -> Result<HeaderValue, HeaderQueryError> {
        let plan = self
            .constant_plans
            .get(&id)
            .cloned()
            .expect("queried constant identity must retain its closed plan");
        self.resolve_constant_dependencies(query, plan.dependencies())?;
        let value = evaluate_expression_plan(&plan, |dependency| {
            match query.completed(&HeaderValueKey::Constant(dependency)) {
                Some(HeaderValue::Constant(value)) => Some(value.clone()),
                _ => None,
            }
        })
        .map_err(evaluation_error)
        .map_err(DependencyQueryError::computation)?;
        Ok(HeaderValue::Constant(value))
    }

    fn array_length(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        id: ConstantExpressionId,
    ) -> Result<HeaderValue, HeaderQueryError> {
        let plan = self
            .array_length_plans
            .get(&id)
            .cloned()
            .expect("queried array-length identity must retain its closed plan");
        self.resolve_constant_dependencies(query, plan.dependencies())?;
        let value = evaluate_expression_plan(&plan, |dependency| {
            match query.completed(&HeaderValueKey::Constant(dependency)) {
                Some(HeaderValue::Constant(value)) => Some(value.clone()),
                _ => None,
            }
        })
        .map_err(evaluation_error)
        .map_err(DependencyQueryError::computation)?;
        let ConstantValue::Integer(value) = value else {
            return Err(DependencyQueryError::computation(rule_at(
                DefinitionRule::CompileTimeTypeMismatch,
                plan.origin(),
            )));
        };
        let length = u64::try_from(value).map_err(|_| {
            DependencyQueryError::computation(rule_at(
                DefinitionRule::CompileTimeArithmeticFailure,
                plan.origin(),
            ))
        })?;
        Ok(HeaderValue::ArrayLength(length))
    }

    fn static_value(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        id: StaticId,
    ) -> Result<HeaderValue, HeaderQueryError> {
        let source = self
            .static_sources
            .get(&id)
            .copied()
            .expect("queried static identity must retain its bound source");
        let origin = SyntaxOrigin::Node(source.initializer);
        let expected = self.frozen_type(query, source.ty, &mut HashSet::new(), origin)?;
        let (file, tree) = syntax_input(self.bindings, self.source_ids, source.initializer)
            .map_err(DependencyQueryError::computation)?;
        let syntax = nocter_syntax::BoundSyntax::new(file, tree).ok_or_else(|| {
            DependencyQueryError::computation(inconsistent_node(source.initializer))
        })?;
        let plan = plan_frozen_expression(
            self.target,
            syntax,
            source.initializer,
            &expected,
            self.resolver,
        )
        .map_err(plan_error)
        .map_err(DependencyQueryError::computation)?;
        self.resolve_constant_dependencies(query, &plan.dependencies())?;
        let value = evaluate_frozen_expression_plan(&plan, &mut |dependency| match query
            .completed(&HeaderValueKey::Constant(dependency))
        {
            Some(HeaderValue::Constant(value)) => Some(value.clone()),
            _ => None,
        })
        .map_err(evaluation_error)
        .map_err(DependencyQueryError::computation)?;
        Ok(HeaderValue::Static(value))
    }

    fn resolve_constant_dependencies(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        dependencies: &[(ConstantId, SyntaxOrigin)],
    ) -> Result<(), HeaderQueryError> {
        for &(dependency, origin) in dependencies {
            let value =
                self.resolve_dependency(query, HeaderValueKey::Constant(dependency), origin)?;
            if !matches!(value.as_ref(), HeaderValue::Constant(_)) {
                return Err(DependencyQueryError::computation(inconsistent_origin(
                    origin,
                )));
            }
        }
        Ok(())
    }

    fn resolve_dependency(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        key: HeaderValueKey,
        origin: SyntaxOrigin,
    ) -> Result<std::sync::Arc<HeaderValue>, HeaderQueryError> {
        match query.resolve(self, key) {
            Err(DependencyQueryError::Cycle(_)) => Err(DependencyQueryError::computation(rule_at(
                DefinitionRule::CompileTimeCycle,
                origin,
            ))),
            result => result,
        }
    }

    fn frozen_type(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        ty: BoundTypeId,
        active_aliases: &mut HashSet<nocter_model::TypeAliasId>,
        origin: SyntaxOrigin,
    ) -> Result<FrozenType, HeaderQueryError> {
        let kind = self
            .bindings
            .kinds
            .get(ty.index())
            .cloned()
            .ok_or_else(|| DependencyQueryError::computation(inconsistent_origin(origin)))?;
        match kind {
            BoundTypeKind::FixedArray {
                element,
                length: length_node,
            } => {
                let element = self.frozen_type(query, element, active_aliases, origin)?;
                let id = self
                    .bindings
                    .array_expression_ids
                    .get(&length_node)
                    .copied()
                    .ok_or_else(|| {
                        DependencyQueryError::computation(inconsistent_node(length_node))
                    })?;
                let value = self.resolve_dependency(
                    query,
                    HeaderValueKey::ArrayLength(id),
                    SyntaxOrigin::Node(length_node),
                )?;
                let HeaderValue::ArrayLength(length) = value.as_ref() else {
                    return Err(DependencyQueryError::computation(inconsistent_node(
                        length_node,
                    )));
                };
                let length = usize::try_from(*length).map_err(|_| {
                    DependencyQueryError::computation(rule_at(
                        DefinitionRule::CompileTimeArithmeticFailure,
                        SyntaxOrigin::Node(length_node),
                    ))
                })?;
                Ok(FrozenType::FixedArray {
                    element: Box::new(element),
                    length,
                })
            }
            BoundTypeKind::Alias {
                definition,
                arguments,
            } if arguments.is_empty() && active_aliases.insert(definition) => {
                let result = match self.bindings.alias_targets.get(&definition).copied() {
                    Some(target) => self.frozen_type(query, target, active_aliases, origin),
                    None => Err(DependencyQueryError::computation(inconsistent_origin(
                        origin,
                    ))),
                };
                active_aliases.remove(&definition);
                result
            }
            _ => self
                .resolver
                .scalar_type(ty, active_aliases)
                .map(FrozenType::Scalar)
                .ok_or_else(|| {
                    DependencyQueryError::computation(rule_at(
                        DefinitionRule::InvalidCompileTimeValueType,
                        origin,
                    ))
                }),
        }
    }

    fn query_error(&self, key: HeaderValueKey, error: HeaderQueryError) -> HeaderDefinitionError {
        match error {
            DependencyQueryError::Computation(error) => error,
            DependencyQueryError::Cycle(_) => rule_at(
                DefinitionRule::CompileTimeCycle,
                self.key_origin(key)
                    .expect("queried header value must retain a diagnostic origin"),
            ),
        }
    }

    fn key_origin(&self, key: HeaderValueKey) -> Option<SyntaxOrigin> {
        match key {
            HeaderValueKey::Constant(id) => self
                .constant_plans
                .get(&id)
                .map(ConstantExpressionPlan::origin),
            HeaderValueKey::ArrayLength(id) => self
                .array_length_plans
                .get(&id)
                .map(ConstantExpressionPlan::origin),
            HeaderValueKey::Static(id) => self
                .static_sources
                .get(&id)
                .map(|source| SyntaxOrigin::Node(source.initializer)),
        }
    }
}

fn collect_static_sources(
    bindings: &PreparedTypeBindings<'_>,
) -> Result<HashMap<StaticId, StaticSource>, HeaderDefinitionError> {
    let reserved = &bindings.namespaces.imports.generics.headers.reserved;
    let mut result = HashMap::new();
    for (index, entity) in reserved.entities().iter().copied().enumerate() {
        let Some(ReservedEntity::Static(id)) = entity else {
            continue;
        };
        let declaration = SurfaceDeclarationId::from_index(index);
        let surface = reserved.declarations[index];
        if surface.kind() != SurfaceDeclarationKind::Static {
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
                StaticSource {
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

fn collect_sources(
    bindings: &PreparedTypeBindings<'_>,
) -> Result<HashMap<ConstantId, ConstantSource>, HeaderDefinitionError> {
    let reserved = &bindings.namespaces.imports.generics.headers.reserved;
    let mut result = HashMap::new();
    for (index, entity) in reserved.entities().iter().copied().enumerate() {
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
        let source = self
            .sources
            .get(&id)
            .ok_or_else(|| inconsistent_node(node))?;
        let ty = self
            .scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                rule_at(
                    DefinitionRule::InvalidCompileTimeValueType,
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
                return Err(inconsistent_node(node));
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
            BoundTypeKind::Builtin(BuiltinType::Char) => Some(ConstantScalarType::Character),
            BoundTypeKind::Builtin(BuiltinType::F32) => {
                Some(ConstantScalarType::Float(FloatFormat::Binary32))
            }
            BoundTypeKind::Builtin(BuiltinType::F64) => {
                Some(ConstantScalarType::Float(FloatFormat::Binary64))
            }
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
                let token =
                    first_direct_token(tree, node).ok_or_else(|| inconsistent_node(node))?;
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
                let nodes = child_nodes(tree, node);
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
                let token = first_direct_token(tree, nodes[1])
                    .filter(|token| token.kind() == TokenKind::Identifier)
                    .ok_or_else(|| inconsistent_node(node))?;
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
            .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))
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
            .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))
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
}

fn inconsistent_node(node: NodeId) -> HeaderDefinitionError {
    HeaderDefinitionError::InconsistentSource(node.source())
}

fn inconsistent_origin(origin: SyntaxOrigin) -> HeaderDefinitionError {
    match origin {
        SyntaxOrigin::Node(node) => inconsistent_node(node),
        SyntaxOrigin::Token(token) => HeaderDefinitionError::InconsistentSource(token.source()),
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

fn plan_error(error: ConstantPlanError<HeaderDefinitionError>) -> HeaderDefinitionError {
    match error {
        ConstantPlanError::Context(error) => error,
        ConstantPlanError::Rule { rule, origin } => rule_at(
            match rule {
                ConstantPlanRule::NonConstantExpression => DefinitionRule::NonConstantExpression,
                ConstantPlanRule::TypeMismatch => DefinitionRule::CompileTimeTypeMismatch,
            },
            origin,
        ),
        ConstantPlanError::InvalidSyntax(node) => inconsistent_node(node),
    }
}

fn evaluation_error(error: ConstantEvaluationError) -> HeaderDefinitionError {
    match error.rule() {
        ConstantEvaluationRule::ArithmeticFailure => {
            rule_at(DefinitionRule::CompileTimeArithmeticFailure, error.origin())
        }
        ConstantEvaluationRule::MissingConstant | ConstantEvaluationRule::InvalidPlan => {
            match error.origin() {
                SyntaxOrigin::Node(node) => inconsistent_node(node),
                SyntaxOrigin::Token(token) => {
                    HeaderDefinitionError::InconsistentSource(token.source())
                }
            }
        }
    }
}

fn project_references(
    bindings: &mut PreparedTypeBindings<'_>,
    references: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
) {
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
            .insert(semantic_entity(entity), SourceRole::Reference, origin);
    }
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

const fn semantic_entity(entity: ExportedEntity) -> SemanticEntity {
    match entity {
        ExportedEntity::BuiltinType(builtin) => SemanticEntity::BuiltinType(builtin),
        ExportedEntity::Module(id) => SemanticEntity::Module(id),
        ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        ExportedEntity::Constant(id) => SemanticEntity::Constant(id),
        ExportedEntity::Static(id) => SemanticEntity::Static(id),
        ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}
