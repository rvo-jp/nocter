use std::collections::{HashMap, HashSet};

use nocter_constant_evaluation::{
    ConstantEvaluationError, ConstantEvaluationRule, ConstantExpressionPlan, ConstantPlanError,
    ConstantPlanRule, ConstantReference, ConstantResolver, ConstantScalarType,
    DependencyComputation, DependencyQuery, DependencyQueryError, FloatFormat,
    evaluate_expression_plan, plan_expression,
};
use nocter_declarations::ExportedEntity;
use nocter_model::{
    BorrowCapability, BuiltinType, CompilationTarget, ConstantExpressionId, ConstantId,
    ConstantValue, ModuleId,
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

use super::super::{BoundTypeId, BoundTypeKind, PreparedStructuralConstant, PreparedTypeBindings};

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

/// Plans and evaluates the scalar constants available to structural type normalization.
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
    let evaluated = evaluate_header_values(target, &bindings, &source_ids, &sources)?;

    let structural_constants = evaluated
        .constants
        .into_iter()
        .map(|(id, value)| {
            (
                id,
                PreparedStructuralConstant {
                    declaration: sources[&id].declaration,
                    value,
                },
            )
        })
        .collect();
    project_references(&mut bindings, evaluated.reference_projections);
    project_generic_references(&mut bindings, evaluated.generic_reference_projections);
    bindings.structural_constants = structural_constants;
    bindings.usize_terms = evaluated.usize_terms;
    Ok(bindings)
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HeaderValueKey {
    Constant(ConstantId),
    UsizeExpression(ConstantExpressionId),
}

#[derive(Clone, Debug)]
enum HeaderValue {
    Constant(ConstantValue),
    UsizeTerm(nocter_model::UsizeTerm),
}

struct EvaluatedHeaderValues {
    constants: HashMap<ConstantId, ConstantValue>,
    usize_terms: HashMap<ConstantExpressionId, nocter_model::UsizeTerm>,
    reference_projections: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
    generic_reference_projections:
        HashMap<SyntaxToken, (nocter_model::GenericParameterId, SourceOrigin)>,
}

struct HeaderValueComputation {
    constant_plans: HashMap<ConstantId, ConstantExpressionPlan>,
    usize_expression_plans: HashMap<ConstantExpressionId, ConstantExpressionPlan>,
    symbolic_usize_terms: HashMap<ConstantExpressionId, nocter_model::UsizeTerm>,
}

fn evaluate_header_values(
    target: CompilationTarget,
    bindings: &PreparedTypeBindings<'_>,
    source_ids: &HashMap<SourceId, crate::SurfaceSourceId>,
    constant_sources: &HashMap<ConstantId, ConstantSource>,
) -> Result<EvaluatedHeaderValues, HeaderDefinitionError> {
    let mut constant_plans = HashMap::new();
    let mut reference_projections = HashMap::new();
    let mut constant_ids = constant_sources.keys().copied().collect::<Vec<_>>();
    constant_ids.sort_unstable();
    for id in &constant_ids {
        let source = constant_sources[id];
        let mut resolver = HeaderResolver::new(bindings, constant_sources, source_ids);
        let Some(expected) = resolver.scalar_type(source.ty, &mut HashSet::new()) else {
            continue;
        };
        let (file, tree) = syntax_input(bindings, source_ids, source.initializer)?;
        if contains_unbound_type(tree, source.initializer, &bindings.roots) {
            continue;
        }
        let syntax = nocter_syntax::BoundSyntax::new(file, tree)
            .ok_or_else(|| inconsistent_node(source.initializer))?;
        match plan_expression(target, syntax, source.initializer, expected, &mut resolver) {
            Ok(plan) => {
                constant_plans.insert(*id, plan);
            }
            Err(error) if optional_plan_failure(&error) => {}
            Err(error) => return Err(plan_error(error)),
        }
    }

    let usize_ty = ConstantScalarType::Integer(BuiltinType::Usize);
    let mut usize_expression_plans = HashMap::new();
    let mut symbolic_usize_terms = HashMap::new();
    let mut generic_reference_projections = HashMap::new();
    let mut usize_expression_ids = Vec::new();
    for (id, expression) in bindings.usize_expressions.iter() {
        usize_expression_ids.push(id);
        let expression = *expression;
        if let Some((parameter, token, origin)) =
            constant_parameter_reference(bindings, source_ids, expression)?
        {
            symbolic_usize_terms.insert(id, nocter_model::UsizeTerm::Parameter(parameter));
            generic_reference_projections.insert(token, (parameter, origin));
            continue;
        }
        let (file, tree) = syntax_input(bindings, source_ids, expression)?;
        let syntax = nocter_syntax::BoundSyntax::new(file, tree)
            .ok_or_else(|| inconsistent_node(expression))?;
        let mut resolver = HeaderResolver::new(bindings, constant_sources, source_ids);
        let plan = plan_expression(target, syntax, expression, usize_ty, &mut resolver)
            .map_err(plan_error)?;
        merge_reference_projections(&mut reference_projections, resolver.reference_projections)?;
        usize_expression_plans.insert(id, plan);
    }

    let mut computation = HeaderValueComputation {
        constant_plans,
        usize_expression_plans,
        symbolic_usize_terms,
    };
    let mut query = DependencyQuery::default();
    // Resolve structural usize expressions first so correctness cannot depend on an eager
    // constants-first pass. Their plans request constants through this same authority.
    for key in usize_expression_ids
        .iter()
        .copied()
        .map(HeaderValueKey::UsizeExpression)
    {
        if let Err(error) = query.resolve(&mut computation, key) {
            return Err(computation.query_error(key, error));
        }
    }

    // Structural eligibility is intentionally best-effort for ordinary constant declarations.
    // Their checked initializer plans remain the final semantic authority. A failure here only
    // becomes a declaration error when an actual header array length demands that constant.
    for id in &constant_ids {
        if !computation.constant_plans.contains_key(id) {
            continue;
        }
        let key = HeaderValueKey::Constant(*id);
        if let Err(error) = query.resolve(&mut computation, key) {
            let error = computation.query_error(key, error);
            if !matches!(error, HeaderDefinitionError::Rule(_)) {
                return Err(error);
            }
        }
    }

    let constants = constant_ids
        .into_iter()
        .filter_map(|id| match query.completed(&HeaderValueKey::Constant(id)) {
            Some(HeaderValue::Constant(value)) => Some((id, value.clone())),
            Some(HeaderValue::UsizeTerm(_)) | None => None,
        })
        .collect::<HashMap<_, _>>();
    let usize_terms = usize_expression_ids
        .into_iter()
        .map(
            |id| match query.completed(&HeaderValueKey::UsizeExpression(id)) {
                Some(HeaderValue::UsizeTerm(value)) => Ok((id, value.clone())),
                _ => Err(inconsistent_node(
                    *bindings
                        .usize_expressions
                        .get(id)
                        .expect("queried usize expression must retain its bound syntax"),
                )),
            },
        )
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(EvaluatedHeaderValues {
        constants,
        usize_terms,
        reference_projections,
        generic_reference_projections,
    })
}

type HeaderQueryError = DependencyQueryError<HeaderValueKey, HeaderDefinitionError>;

impl DependencyComputation<HeaderValueKey, HeaderValue, HeaderDefinitionError>
    for HeaderValueComputation
{
    fn compute(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        key: HeaderValueKey,
    ) -> Result<HeaderValue, HeaderQueryError> {
        match key {
            HeaderValueKey::Constant(id) => self.constant(query, id),
            HeaderValueKey::UsizeExpression(id) => self.usize_expression(query, id),
        }
    }
}

impl HeaderValueComputation {
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

    fn usize_expression(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        id: ConstantExpressionId,
    ) -> Result<HeaderValue, HeaderQueryError> {
        if let Some(term) = self.symbolic_usize_terms.get(&id) {
            return Ok(HeaderValue::UsizeTerm(*term));
        }
        let plan = self
            .usize_expression_plans
            .get(&id)
            .cloned()
            .expect("queried usize expression must retain its closed plan");
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
        Ok(HeaderValue::UsizeTerm(length.into()))
    }

    fn resolve_constant_dependencies(
        &mut self,
        query: &mut DependencyQuery<HeaderValueKey, HeaderValue>,
        dependencies: &[(ConstantId, SyntaxOrigin)],
    ) -> Result<(), HeaderQueryError> {
        for &(dependency, origin) in dependencies {
            if !self.constant_plans.contains_key(&dependency) {
                return Err(DependencyQueryError::computation(rule_at(
                    DefinitionRule::NonConstantExpression,
                    origin,
                )));
            }
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
            HeaderValueKey::UsizeExpression(id) => self
                .usize_expression_plans
                .get(&id)
                .map(ConstantExpressionPlan::origin),
        }
    }
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
    fn new<'a, 'syntax>(
        bindings: &'a PreparedTypeBindings<'syntax>,
        sources: &'a HashMap<ConstantId, ConstantSource>,
        source_ids: &'a HashMap<SourceId, crate::SurfaceSourceId>,
    ) -> HeaderResolver<'a, 'syntax> {
        HeaderResolver {
            bindings,
            sources,
            source_ids,
            references: HashMap::new(),
            reference_projections: HashMap::new(),
        }
    }

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

fn constant_parameter_reference(
    bindings: &PreparedTypeBindings<'_>,
    source_ids: &HashMap<SourceId, crate::SurfaceSourceId>,
    expression: NodeId,
) -> Result<
    Option<(nocter_model::GenericParameterId, SyntaxToken, SourceOrigin)>,
    HeaderDefinitionError,
> {
    let (_, tree) = syntax_input(bindings, source_ids, expression)?;
    let expression_range = tree
        .node(expression)
        .ok_or_else(|| inconsistent_node(expression))?
        .range();
    let mut references = Vec::new();
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|syntax| syntax.kind() == NodeKind::ReferenceExpression)
        {
            references.push(node);
        }
        pending.extend(child_nodes(tree, node));
    }
    let [reference] = references.as_slice() else {
        return Ok(None);
    };
    if tree
        .node(*reference)
        .is_none_or(|syntax| syntax.range() != expression_range)
    {
        return Ok(None);
    }
    let token = first_direct_token(tree, *reference)
        .filter(|token| token.kind() == TokenKind::Identifier)
        .ok_or_else(|| inconsistent_node(*reference))?;
    let spelling = bindings
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .source_map
        .get(token.source())
        .and_then(|source| source.text_at(token.range()))
        .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))?;
    let symbol = bindings
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .symbols()
        .get(spelling)
        .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))?;
    let declaration = bindings
        .usize_expression_declarations
        .get(&expression)
        .copied()
        .ok_or_else(|| inconsistent_node(expression))?;
    let Some(parameter) = bindings
        .namespaces
        .imports
        .generics
        .lookup(declaration, symbol)
    else {
        return Ok(None);
    };
    let metadata = bindings
        .namespaces
        .imports
        .generics
        .headers
        .reserved
        .program
        .declarations()
        .generic_parameter(parameter)
        .ok_or_else(|| inconsistent_node(expression))?;
    if metadata.domain() != nocter_declarations::GenericParameterDomain::UsizeConstant {
        return Ok(None);
    }
    let source = source_ids
        .get(&token.source())
        .and_then(|source| {
            bindings
                .namespaces
                .imports
                .generics
                .headers
                .reserved
                .sources
                .get(source.index())
        })
        .ok_or(HeaderDefinitionError::InconsistentSource(token.source()))?;
    let origin = SourceOrigin::from_token(source.syntax(), token)
        .map_err(|_| HeaderDefinitionError::InconsistentSource(token.source()))?;
    Ok(Some((parameter, token, origin)))
}

fn contains_unbound_type(
    tree: &SyntaxTree,
    root: NodeId,
    bound_types: &HashMap<NodeId, BoundTypeId>,
) -> bool {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree
            .node(node)
            .is_some_and(|node| node.kind() == NodeKind::Type)
            && !bound_types.contains_key(&node)
        {
            return true;
        }
        pending.extend(child_nodes(tree, node));
    }
    false
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

fn optional_plan_failure(error: &ConstantPlanError<HeaderDefinitionError>) -> bool {
    matches!(
        error,
        ConstantPlanError::Rule { .. } | ConstantPlanError::Context(HeaderDefinitionError::Rule(_))
    )
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

fn project_generic_references(
    bindings: &mut PreparedTypeBindings<'_>,
    references: HashMap<SyntaxToken, (nocter_model::GenericParameterId, SourceOrigin)>,
) {
    let mut references = references.into_iter().collect::<Vec<_>>();
    references.sort_unstable_by_key(|(token, _)| {
        (token.source(), token.range().start(), token.range().end())
    });
    for (_, (parameter, origin)) in references {
        bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .source_index
            .insert(
                SemanticEntity::GenericParameter(parameter),
                SourceRole::Reference,
                origin,
            );
    }
}

fn merge_reference_projections(
    destination: &mut HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
    source: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
) -> Result<(), HeaderDefinitionError> {
    for (token, projection) in source {
        if destination
            .insert(token, projection)
            .is_some_and(|(existing, _)| existing != projection.0)
        {
            return Err(HeaderDefinitionError::InconsistentSource(token.source()));
        }
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
