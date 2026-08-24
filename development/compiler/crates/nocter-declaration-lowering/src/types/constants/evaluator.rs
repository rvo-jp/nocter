use std::collections::{HashMap, HashSet};

use nocter_declarations::ExportedEntity;
use nocter_model::{BorrowCapability, BuiltinType, ConstantId, ConstantValue, ModuleId};
use nocter_source::SourceId;
use nocter_source_index::{SemanticEntity, SourceOrigin, SourceRole, SyntaxOrigin};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxToken, SyntaxTree, TokenKind,
    decode_plain_string_expression,
};

use crate::{
    DefinitionRule, DefinitionViolation, HeaderDefinitionError, ReservedEntity,
    SurfaceDeclarationId, SurfaceDeclarationKind,
};

use super::super::{BoundTypeId, BoundTypeKind, PreparedConstantValue, PreparedTypeBindings};

use super::support::{
    direct_node, direct_nodes, direct_punctuation, direct_token, expression_children, integer_spec,
    one_expression_child, parse_integer, shift,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScalarType {
    Bool,
    Integer(BuiltinType),
    Text,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedValue {
    ty: ScalarType,
    value: ConstantValue,
}

#[derive(Clone, Copy)]
pub(super) struct ConstantSource {
    pub(super) declaration: SurfaceDeclarationId,
    pub(super) initializer: NodeId,
    pub(super) ty: BoundTypeId,
}

pub(super) struct Evaluator<'a, 'syntax> {
    pub(super) bindings: &'a PreparedTypeBindings<'syntax>,
    pub(super) sources: HashMap<ConstantId, ConstantSource>,
    values: HashMap<ConstantId, TypedValue>,
    active: HashSet<ConstantId>,
    source_ids: HashMap<SourceId, crate::SurfaceSourceId>,
    pub(super) expression_types: HashMap<NodeId, ScalarType>,
    pub(super) references: HashMap<NodeId, ConstantId>,
    pub(super) dependencies: HashMap<ConstantId, Vec<(ConstantId, NodeId)>>,
    pub(super) reference_projections: HashMap<SyntaxToken, (ExportedEntity, SourceOrigin)>,
}

/// Evaluates every header constant before structural type normalization.
///
/// The resulting values are the sole constant-expression authority shared by declaration
/// freezing and fixed-array normalization. Neither later phase reads initializer syntax again.
///
/// # Errors
///
/// Returns the exact constant rule violation or an internal declaration-graph inconsistency.
pub fn evaluate(
    mut bindings: PreparedTypeBindings<'_>,
) -> Result<PreparedTypeBindings<'_>, HeaderDefinitionError> {
    let sources = collect_sources(&bindings)?;
    let ids = sources.keys().copied().collect::<Vec<_>>();
    let mut evaluator = Evaluator {
        bindings: &bindings,
        sources,
        values: HashMap::new(),
        active: HashSet::new(),
        source_ids: bindings
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
            .collect(),
        expression_types: HashMap::new(),
        references: HashMap::new(),
        dependencies: HashMap::new(),
        reference_projections: HashMap::new(),
    };

    let array_expressions = collect_array_expressions(&bindings);
    let usize_ty = ScalarType::Integer(BuiltinType::Usize);

    for id in &ids {
        evaluator.analyze_constant(*id)?;
    }
    for expression in &array_expressions {
        evaluator.analyze_expression(*expression, Some(usize_ty), None)?;
    }
    evaluator.ensure_acyclic()?;
    for id in ids {
        evaluator.evaluate_constant(id)?;
    }

    let mut array_lengths = HashMap::with_capacity(array_expressions.len());
    for expression in array_expressions {
        let value = evaluator.evaluate_expression(expression, Some(usize_ty))?;
        let ConstantValue::Integer(value) = value.value else {
            return Err(Evaluator::rule_at(
                DefinitionRule::ConstantTypeMismatch,
                SyntaxOrigin::Node(expression),
            ));
        };
        let length = u64::try_from(value).map_err(|_| {
            Evaluator::rule_at(
                DefinitionRule::ConstantArithmeticFailure,
                SyntaxOrigin::Node(expression),
            )
        })?;
        array_lengths.insert(expression, length);
    }

    let Evaluator {
        values,
        sources,
        reference_projections,
        ..
    } = evaluator;
    let values = values
        .into_iter()
        .map(|(id, typed)| {
            let declaration = sources[&id].declaration;
            (
                id,
                PreparedConstantValue {
                    declaration,
                    value: typed.value,
                },
            )
        })
        .collect();
    project_references(&mut bindings, reference_projections)?;
    bindings.constant_values = values;
    bindings.array_lengths = array_lengths;
    Ok(bindings)
}

fn collect_array_expressions(bindings: &PreparedTypeBindings<'_>) -> Vec<NodeId> {
    let reserved = &bindings.namespaces.imports.generics.headers.reserved;
    let mut expressions = reserved
        .sources
        .iter()
        .flat_map(|source| {
            let tree = source.syntax();
            tree.nodes().filter_map(move |(node, syntax)| {
                (syntax.kind() == NodeKind::FixedArrayType)
                    .then(|| direct_node(tree, node, NodeKind::Expression))
                    .flatten()
            })
        })
        .collect::<Vec<_>>();
    expressions.sort_unstable_by_key(|node| (node.source(), node.index()));
    expressions.dedup();
    expressions
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

impl Evaluator<'_, '_> {
    fn evaluate_constant(&mut self, id: ConstantId) -> Result<TypedValue, HeaderDefinitionError> {
        if let Some(value) = self.values.get(&id) {
            return Ok(value.clone());
        }
        let source = self
            .sources
            .get(&id)
            .copied()
            .ok_or_else(|| self.rule(DefinitionRule::NonConstantExpression, id))?;
        let expected = self
            .scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                Self::rule_at(
                    DefinitionRule::InvalidConstantType,
                    SyntaxOrigin::Node(source.initializer),
                )
            })?;
        if !self.active.insert(id) {
            return Err(self.rule(DefinitionRule::ConstantCycle, id));
        }
        let value = self.evaluate_expression(source.initializer, Some(expected));
        self.active.remove(&id);
        let value = value?;
        if value.ty != expected {
            return Err(Self::rule_at(
                DefinitionRule::ConstantTypeMismatch,
                SyntaxOrigin::Node(source.initializer),
            ));
        }
        self.values.insert(id, value.clone());
        Ok(value)
    }

    fn evaluate_expression(
        &mut self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let planned = self
            .expression_types
            .get(&node)
            .copied()
            .ok_or_else(|| self.internal(node))?;
        if expected.is_some_and(|expected| expected != planned) {
            return Err(Self::mismatch_node(node));
        }
        let expected = Some(planned);
        let tree = self.tree(node)?;
        let kind = tree
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or_else(|| self.internal(node))?;
        match kind {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child = one_expression_child(tree, node).ok_or_else(|| {
                    Self::rule_at(
                        DefinitionRule::NonConstantExpression,
                        SyntaxOrigin::Node(node),
                    )
                })?;
                self.evaluate_expression(child, expected)
            }
            NodeKind::ScalarLiteral => self.evaluate_scalar(node, expected),
            NodeKind::StringExpression => self.evaluate_text(node, expected),
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                let id = self
                    .references
                    .get(&node)
                    .copied()
                    .ok_or_else(|| self.internal(node))?;
                let value = self.evaluate_constant(id)?;
                if expected.is_some_and(|expected| expected != value.ty) {
                    return Err(Self::rule_at(
                        DefinitionRule::ConstantTypeMismatch,
                        SyntaxOrigin::Node(node),
                    ));
                }
                Ok(value)
            }
            NodeKind::UnaryExpression => self.evaluate_unary(node, expected),
            NodeKind::LogicalAndExpression
            | NodeKind::LogicalOrExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => self.evaluate_binary(node, expected),
            NodeKind::ConversionExpression => self.evaluate_conversion(node, expected),
            _ => Err(Self::rule_at(
                DefinitionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            )),
        }
    }

    fn evaluate_scalar(
        &self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let token = direct_token(tree, node).ok_or_else(|| self.internal(node))?;
        match token.kind() {
            TokenKind::Keyword(Keyword::True | Keyword::False) => {
                if expected.is_some_and(|expected| expected != ScalarType::Bool) {
                    return Err(Self::mismatch(token));
                }
                Ok(TypedValue {
                    ty: ScalarType::Bool,
                    value: ConstantValue::Bool(token.kind() == TokenKind::Keyword(Keyword::True)),
                })
            }
            TokenKind::IntegerLiteral => {
                let ty = expected.unwrap_or(ScalarType::Integer(BuiltinType::I32));
                let ScalarType::Integer(builtin) = ty else {
                    return Err(Self::mismatch(token));
                };
                let value = i128::from(
                    parse_integer(self.token_text(token)?)
                        .ok_or_else(|| Self::arithmetic(SyntaxOrigin::Token(token)))?,
                );
                if !integer_spec(builtin).is_some_and(|spec| spec.contains(value)) {
                    return Err(Self::arithmetic(SyntaxOrigin::Token(token)));
                }
                Ok(TypedValue {
                    ty,
                    value: ConstantValue::Integer(value),
                })
            }
            _ => Err(Self::rule_at(
                DefinitionRule::NonConstantExpression,
                SyntaxOrigin::Token(token),
            )),
        }
    }

    fn evaluate_text(
        &self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        if expected.is_some_and(|expected| expected != ScalarType::Text) {
            return Err(Self::rule_at(
                DefinitionRule::ConstantTypeMismatch,
                SyntaxOrigin::Node(node),
            ));
        }
        let tree = self.tree(node)?;
        let source = self
            .bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .source_map
            .get(node.source())
            .ok_or(HeaderDefinitionError::InconsistentSource(node.source()))?;
        let text = decode_plain_string_expression(source, tree, node).ok_or_else(|| {
            Self::rule_at(
                DefinitionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            )
        })?;
        Ok(TypedValue {
            ty: ScalarType::Text,
            value: ConstantValue::Text(text),
        })
    }

    fn evaluate_unary(
        &mut self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let operator = direct_punctuation(tree, node).ok_or_else(|| self.internal(node))?;
        let operand = one_expression_child(tree, node).ok_or_else(|| self.internal(node))?;
        match operator {
            Punctuation::Bang => {
                if expected.is_some_and(|expected| expected != ScalarType::Bool) {
                    return Err(Self::mismatch_node(node));
                }
                let value = self.evaluate_expression(operand, Some(ScalarType::Bool))?;
                let ConstantValue::Bool(value) = value.value else {
                    return Err(Self::mismatch_node(node));
                };
                Ok(TypedValue {
                    ty: ScalarType::Bool,
                    value: ConstantValue::Bool(!value),
                })
            }
            Punctuation::Minus => {
                let ty = expected.unwrap_or(ScalarType::Integer(BuiltinType::I32));
                let ScalarType::Integer(builtin) = ty else {
                    return Err(Self::mismatch_node(node));
                };
                let Some(spec) = integer_spec(builtin).filter(|spec| spec.signed) else {
                    return Err(Self::mismatch_node(node));
                };
                let value = if let Some(magnitude) = self.integer_literal(operand)? {
                    let maximum_magnitude = spec.maximum + 1;
                    if i128::from(magnitude) > maximum_magnitude {
                        return Err(Self::arithmetic(SyntaxOrigin::Node(node)));
                    }
                    -i128::from(magnitude)
                } else {
                    let value = self.evaluate_expression(operand, Some(ty))?;
                    let ConstantValue::Integer(value) = value.value else {
                        return Err(Self::mismatch_node(node));
                    };
                    value
                        .checked_neg()
                        .ok_or_else(|| Self::arithmetic(SyntaxOrigin::Node(node)))?
                };
                if !spec.contains(value) {
                    return Err(Self::arithmetic(SyntaxOrigin::Node(node)));
                }
                Ok(TypedValue {
                    ty,
                    value: ConstantValue::Integer(value),
                })
            }
            _ => Err(Self::rule_at(
                DefinitionRule::NonConstantExpression,
                SyntaxOrigin::Node(node),
            )),
        }
    }

    fn evaluate_binary(
        &mut self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let operands = expression_children(tree, node);
        if operands.len() != 2 {
            return Err(self.internal(node));
        }
        let operator = direct_punctuation(tree, node).ok_or_else(|| self.internal(node))?;
        match operator {
            Punctuation::LogicalAnd | Punctuation::LogicalOr => {
                if expected.is_some_and(|expected| expected != ScalarType::Bool) {
                    return Err(Self::mismatch_node(node));
                }
                let left = self.evaluate_expression(operands[0], Some(ScalarType::Bool))?;
                let ConstantValue::Bool(left) = left.value else {
                    return Err(Self::mismatch_node(node));
                };
                let value = if operator == Punctuation::LogicalAnd {
                    left && self.boolean_expression(operands[1])?
                } else {
                    left || self.boolean_expression(operands[1])?
                };
                Ok(TypedValue {
                    ty: ScalarType::Bool,
                    value: ConstantValue::Bool(value),
                })
            }
            Punctuation::EqualEqual | Punctuation::BangEqual => {
                if expected.is_some_and(|expected| expected != ScalarType::Bool) {
                    return Err(Self::mismatch_node(node));
                }
                let left = self.evaluate_expression(operands[0], None)?;
                let right = self.evaluate_expression(operands[1], Some(left.ty))?;
                let equal = left.value == right.value;
                Ok(TypedValue {
                    ty: ScalarType::Bool,
                    value: ConstantValue::Bool(if operator == Punctuation::EqualEqual {
                        equal
                    } else {
                        !equal
                    }),
                })
            }
            Punctuation::Less
            | Punctuation::LessEqual
            | Punctuation::Greater
            | Punctuation::GreaterEqual => {
                if expected.is_some_and(|expected| expected != ScalarType::Bool) {
                    return Err(Self::mismatch_node(node));
                }
                let left = self.evaluate_expression(operands[0], None)?;
                let right = self.evaluate_expression(operands[1], Some(left.ty))?;
                let (ConstantValue::Integer(left), ConstantValue::Integer(right)) =
                    (left.value, right.value)
                else {
                    return Err(Self::mismatch_node(node));
                };
                let value = match operator {
                    Punctuation::Less => left < right,
                    Punctuation::LessEqual => left <= right,
                    Punctuation::Greater => left > right,
                    Punctuation::GreaterEqual => left >= right,
                    _ => unreachable!(),
                };
                Ok(TypedValue {
                    ty: ScalarType::Bool,
                    value: ConstantValue::Bool(value),
                })
            }
            _ => self.evaluate_integer_binary(node, operands[0], operands[1], operator, expected),
        }
    }

    fn evaluate_integer_binary(
        &mut self,
        node: NodeId,
        left_node: NodeId,
        right_node: NodeId,
        operator: Punctuation,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let left = self.evaluate_expression(left_node, expected)?;
        let ScalarType::Integer(builtin) = left.ty else {
            return Err(Self::mismatch_node(node));
        };
        let spec = integer_spec(builtin).ok_or_else(|| Self::mismatch_node(node))?;
        let right = self.evaluate_expression(right_node, Some(left.ty))?;
        let (ConstantValue::Integer(left_value), ConstantValue::Integer(right_value)) =
            (left.value, right.value)
        else {
            return Err(Self::mismatch_node(node));
        };
        let result = match operator {
            Punctuation::Plus => left_value.checked_add(right_value),
            Punctuation::Minus => left_value.checked_sub(right_value),
            Punctuation::Star => left_value.checked_mul(right_value),
            Punctuation::Slash => left_value.checked_div(right_value),
            Punctuation::Percent => left_value.checked_rem(right_value),
            Punctuation::ShiftLeft | Punctuation::ShiftRight => {
                shift(left_value, right_value, operator, spec)
            }
            _ => None,
        }
        .filter(|value| spec.contains(*value))
        .ok_or_else(|| Self::arithmetic(SyntaxOrigin::Node(node)))?;
        Ok(TypedValue {
            ty: left.ty,
            value: ConstantValue::Integer(result),
        })
    }

    fn evaluate_conversion(
        &mut self,
        node: NodeId,
        expected: Option<ScalarType>,
    ) -> Result<TypedValue, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let operand = one_expression_child(tree, node).ok_or_else(|| self.internal(node))?;
        let ty_node = direct_node(tree, node, NodeKind::Type)
            .ok_or(HeaderDefinitionError::MissingType(node))?;
        let bound = self
            .bindings
            .roots
            .get(&ty_node)
            .copied()
            .ok_or(HeaderDefinitionError::MissingType(ty_node))?;
        let target = self
            .scalar_type(bound, &mut HashSet::new())
            .ok_or_else(|| Self::non_constant(node))?;
        let ScalarType::Integer(target_builtin) = target else {
            return Err(Self::non_constant(node));
        };
        if expected.is_some_and(|expected| expected != target) {
            return Err(Self::mismatch_node(node));
        }
        let value = self.evaluate_expression(operand, None)?;
        let ConstantValue::Integer(value) = value.value else {
            return Err(Self::non_constant(node));
        };
        if !integer_spec(target_builtin).is_some_and(|spec| spec.contains(value)) {
            return Err(Self::arithmetic(SyntaxOrigin::Node(node)));
        }
        Ok(TypedValue {
            ty: target,
            value: ConstantValue::Integer(value),
        })
    }

    fn boolean_expression(&mut self, node: NodeId) -> Result<bool, HeaderDefinitionError> {
        let value = self.evaluate_expression(node, Some(ScalarType::Bool))?;
        let ConstantValue::Bool(value) = value.value else {
            return Err(Self::mismatch_node(node));
        };
        Ok(value)
    }

    pub(super) fn scalar_type(
        &self,
        ty: BoundTypeId,
        active_aliases: &mut HashSet<nocter_model::TypeAliasId>,
    ) -> Option<ScalarType> {
        match self.bindings.kinds.get(ty.index())? {
            BoundTypeKind::Builtin(BuiltinType::Bool) => Some(ScalarType::Bool),
            BoundTypeKind::Builtin(builtin) if integer_spec(*builtin).is_some() => {
                Some(ScalarType::Integer(*builtin))
            }
            BoundTypeKind::Borrow {
                capability: BorrowCapability::Readonly,
                referent,
            } if matches!(
                self.bindings.kinds.get(referent.index()),
                Some(BoundTypeKind::Builtin(BuiltinType::Str))
            ) =>
            {
                Some(ScalarType::Text)
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

    pub(super) fn resolve_entity(
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
                    .ok_or_else(|| Self::non_constant(node))?;
                projections.push((token, entity));
                Ok(entity)
            }
            Some(NodeKind::PostfixExpression) => {
                let nodes = direct_nodes(tree, node);
                if nodes.len() != 2
                    || tree.node(nodes[1]).map(nocter_syntax::SyntaxNode::kind)
                        != Some(NodeKind::MemberSuffix)
                {
                    return Err(Self::non_constant(node));
                }
                let ExportedEntity::Module(module) = self.resolve_entity(nodes[0], projections)?
                else {
                    return Err(Self::non_constant(node));
                };
                let token = direct_token(tree, nodes[1])
                    .filter(|token| token.kind() == TokenKind::Identifier)
                    .ok_or_else(|| self.internal(node))?;
                let symbol = self.symbol(token)?;
                let from = self.current_module(node)?;
                let entity = self
                    .bindings
                    .namespaces
                    .lookup_export(from, module, symbol)
                    .ok_or_else(|| Self::non_constant(node))?;
                projections.push((token, entity));
                Ok(entity)
            }
            _ => Err(Self::non_constant(node)),
        }
    }

    pub(super) fn tree(&self, node: NodeId) -> Result<&SyntaxTree, HeaderDefinitionError> {
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
        let source = self.surface_source(node)?;
        self.bindings
            .namespaces
            .imports
            .generics
            .headers
            .reserved
            .module_for_source(source)
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

    fn integer_literal(&self, node: NodeId) -> Result<Option<u64>, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let node = if tree.node(node).is_some_and(|node| {
            matches!(
                node.kind(),
                NodeKind::Expression | NodeKind::GroupedExpression
            )
        }) {
            let Some(child) = one_expression_child(tree, node) else {
                return Ok(None);
            };
            child
        } else {
            node
        };
        if tree
            .node(node)
            .is_none_or(|node| node.kind() != NodeKind::ScalarLiteral)
        {
            return Ok(None);
        }
        let Some(token) =
            direct_token(tree, node).filter(|token| token.kind() == TokenKind::IntegerLiteral)
        else {
            return Ok(None);
        };
        Ok(parse_integer(self.token_text(token)?))
    }

    pub(super) fn rule(&self, rule: DefinitionRule, id: ConstantId) -> HeaderDefinitionError {
        Self::rule_at(rule, SyntaxOrigin::Node(self.sources[&id].initializer))
    }

    pub(super) const fn rule_at(
        rule: DefinitionRule,
        origin: SyntaxOrigin,
    ) -> HeaderDefinitionError {
        HeaderDefinitionError::Rule(DefinitionViolation::new(rule, origin))
    }

    pub(super) const fn non_constant(node: NodeId) -> HeaderDefinitionError {
        Self::rule_at(
            DefinitionRule::NonConstantExpression,
            SyntaxOrigin::Node(node),
        )
    }

    const fn arithmetic(origin: SyntaxOrigin) -> HeaderDefinitionError {
        Self::rule_at(DefinitionRule::ConstantArithmeticFailure, origin)
    }

    const fn mismatch(token: SyntaxToken) -> HeaderDefinitionError {
        Self::rule_at(
            DefinitionRule::ConstantTypeMismatch,
            SyntaxOrigin::Token(token),
        )
    }

    pub(super) const fn mismatch_node(node: NodeId) -> HeaderDefinitionError {
        Self::rule_at(
            DefinitionRule::ConstantTypeMismatch,
            SyntaxOrigin::Node(node),
        )
    }

    pub(super) fn internal(&self, node: NodeId) -> HeaderDefinitionError {
        self.declaration_for_source(node.source()).map_or(
            HeaderDefinitionError::InconsistentSource(node.source()),
            HeaderDefinitionError::InvalidSurface,
        )
    }

    pub(super) fn internal_token(&self, token: SyntaxToken) -> HeaderDefinitionError {
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

const fn semantic_entity(entity: ExportedEntity) -> SemanticEntity {
    match entity {
        ExportedEntity::Module(id) => SemanticEntity::Module(id),
        ExportedEntity::NominalType(id) => SemanticEntity::NominalType(id),
        ExportedEntity::TypeAlias(id) => SemanticEntity::TypeAlias(id),
        ExportedEntity::Interface(id) => SemanticEntity::Interface(id),
        ExportedEntity::Constant(id) => SemanticEntity::Constant(id),
        ExportedEntity::Callable(id) => SemanticEntity::Callable(id),
    }
}
