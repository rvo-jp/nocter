use std::collections::HashSet;

use nocter_declarations::ExportedEntity;
use nocter_model::{BuiltinType, ConstantId};
use nocter_source_index::{SourceOrigin, SyntaxOrigin};
use nocter_syntax::{Keyword, NodeId, NodeKind, Punctuation, TokenKind};

use crate::{DefinitionRule, HeaderDefinitionError};

use super::evaluator::{Evaluator, ScalarType};
use super::support::{
    direct_node, direct_punctuation, direct_token, expression_children, integer_spec,
    one_expression_child,
};

impl Evaluator<'_, '_> {
    pub(super) fn analyze_constant(&mut self, id: ConstantId) -> Result<(), HeaderDefinitionError> {
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
        self.analyze_expression(source.initializer, Some(expected), Some(id))?;
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // This is the exhaustive type contract for const syntax.
    pub(super) fn analyze_expression(
        &mut self,
        node: NodeId,
        expected: Option<ScalarType>,
        owner: Option<ConstantId>,
    ) -> Result<ScalarType, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let kind = tree
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or_else(|| self.internal(node))?;
        let ty = match kind {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child =
                    one_expression_child(tree, node).ok_or_else(|| Self::non_constant(node))?;
                self.analyze_expression(child, expected, owner)?
            }
            NodeKind::ScalarLiteral => {
                let token = direct_token(tree, node).ok_or_else(|| self.internal(node))?;
                match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => ScalarType::Bool,
                    TokenKind::IntegerLiteral => expected
                        .filter(|ty| matches!(ty, ScalarType::Integer(_)))
                        .unwrap_or(ScalarType::Integer(BuiltinType::I32)),
                    _ => return Err(Self::non_constant(node)),
                }
            }
            NodeKind::StringExpression => ScalarType::Text,
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                let id = self.resolve_constant(node)?;
                if let Some(owner) = owner {
                    self.dependencies.entry(owner).or_default().push((id, node));
                }
                self.constant_type(id)?
            }
            NodeKind::UnaryExpression => {
                let operator = direct_punctuation(tree, node).ok_or_else(|| self.internal(node))?;
                let operand =
                    one_expression_child(tree, node).ok_or_else(|| self.internal(node))?;
                match operator {
                    Punctuation::Bang => {
                        self.analyze_expression(operand, Some(ScalarType::Bool), owner)?;
                        ScalarType::Bool
                    }
                    Punctuation::Minus => {
                        let ty = expected
                            .or(self.scalar_hint(operand)?)
                            .unwrap_or(ScalarType::Integer(BuiltinType::I32));
                        if !matches!(ty, ScalarType::Integer(builtin) if integer_spec(builtin).is_some_and(|spec| spec.signed))
                        {
                            return Err(Self::mismatch_node(node));
                        }
                        self.analyze_expression(operand, Some(ty), owner)?;
                        ty
                    }
                    _ => return Err(Self::non_constant(node)),
                }
            }
            NodeKind::LogicalAndExpression | NodeKind::LogicalOrExpression => {
                let operands = self.binary_operands(node)?;
                self.analyze_expression(operands[0], Some(ScalarType::Bool), owner)?;
                self.analyze_expression(operands[1], Some(ScalarType::Bool), owner)?;
                ScalarType::Bool
            }
            NodeKind::EqualityExpression | NodeKind::OrderingExpression => {
                let operands = self.binary_operands(node)?;
                let operand_ty = self
                    .merge_hints(operands[0], operands[1], node)?
                    .unwrap_or(ScalarType::Integer(BuiltinType::I32));
                let left = self.analyze_expression(operands[0], Some(operand_ty), owner)?;
                let right = self.analyze_expression(operands[1], Some(left), owner)?;
                if left != right
                    || kind == NodeKind::OrderingExpression
                        && !matches!(left, ScalarType::Integer(_))
                {
                    return Err(Self::mismatch_node(node));
                }
                ScalarType::Bool
            }
            NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                let ty = expected
                    .or(self.merge_hints(operands[0], operands[1], node)?)
                    .unwrap_or(ScalarType::Integer(BuiltinType::I32));
                if !matches!(ty, ScalarType::Integer(_)) {
                    return Err(Self::mismatch_node(node));
                }
                self.analyze_expression(operands[0], Some(ty), owner)?;
                self.analyze_expression(operands[1], Some(ty), owner)?;
                ty
            }
            NodeKind::ConversionExpression => {
                let operand =
                    one_expression_child(tree, node).ok_or_else(|| self.internal(node))?;
                let ty_node = direct_node(tree, node, NodeKind::Type)
                    .ok_or(HeaderDefinitionError::MissingType(node))?;
                let target = self.scalar_type_node(ty_node, node)?;
                if !matches!(target, ScalarType::Integer(_)) {
                    return Err(Self::non_constant(node));
                }
                let operand_ty = self.analyze_expression(operand, None, owner)?;
                if !matches!(operand_ty, ScalarType::Integer(_)) {
                    return Err(Self::mismatch_node(node));
                }
                target
            }
            _ => return Err(Self::non_constant(node)),
        };
        if expected.is_some_and(|expected| expected != ty) {
            return Err(Self::mismatch_node(node));
        }
        self.expression_types.insert(node, ty);
        Ok(ty)
    }

    fn binary_operands(&self, node: NodeId) -> Result<[NodeId; 2], HeaderDefinitionError> {
        let operands = expression_children(self.tree(node)?, node);
        operands.try_into().map_err(|_| self.internal(node))
    }

    fn merge_hints(
        &mut self,
        left: NodeId,
        right: NodeId,
        origin: NodeId,
    ) -> Result<Option<ScalarType>, HeaderDefinitionError> {
        let left = self.scalar_hint(left)?;
        let right = self.scalar_hint(right)?;
        match (left, right) {
            (Some(left), Some(right)) if left != right => Err(Self::mismatch_node(origin)),
            (Some(ty), _) | (_, Some(ty)) => Ok(Some(ty)),
            (None, None) => Ok(None),
        }
    }

    fn scalar_hint(&mut self, node: NodeId) -> Result<Option<ScalarType>, HeaderDefinitionError> {
        let tree = self.tree(node)?;
        let kind = tree
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or_else(|| self.internal(node))?;
        match kind {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child =
                    one_expression_child(tree, node).ok_or_else(|| Self::non_constant(node))?;
                self.scalar_hint(child)
            }
            NodeKind::ScalarLiteral => {
                let token = direct_token(tree, node).ok_or_else(|| self.internal(node))?;
                Ok(match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => Some(ScalarType::Bool),
                    TokenKind::IntegerLiteral => None,
                    _ => return Err(Self::non_constant(node)),
                })
            }
            NodeKind::StringExpression => Ok(Some(ScalarType::Text)),
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                let id = self.resolve_constant(node)?;
                self.constant_type(id).map(Some)
            }
            NodeKind::UnaryExpression => {
                let operator = direct_punctuation(tree, node).ok_or_else(|| self.internal(node))?;
                if operator == Punctuation::Bang {
                    Ok(Some(ScalarType::Bool))
                } else {
                    let operand =
                        one_expression_child(tree, node).ok_or_else(|| self.internal(node))?;
                    self.scalar_hint(operand)
                }
            }
            NodeKind::LogicalAndExpression
            | NodeKind::LogicalOrExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression => Ok(Some(ScalarType::Bool)),
            NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                self.merge_hints(operands[0], operands[1], node)
            }
            NodeKind::ConversionExpression => {
                let ty_node = direct_node(tree, node, NodeKind::Type)
                    .ok_or(HeaderDefinitionError::MissingType(node))?;
                self.scalar_type_node(ty_node, node).map(Some)
            }
            _ => Err(Self::non_constant(node)),
        }
    }

    fn resolve_constant(&mut self, node: NodeId) -> Result<ConstantId, HeaderDefinitionError> {
        if let Some(id) = self.references.get(&node) {
            return Ok(*id);
        }
        let mut projections = Vec::new();
        let ExportedEntity::Constant(id) = self.resolve_entity(node, &mut projections)? else {
            return Err(Self::non_constant(node));
        };
        for (token, entity) in projections {
            let origin = SourceOrigin::from_token(self.tree(node)?, token)
                .map_err(|_| HeaderDefinitionError::InconsistentSource(token.source()))?;
            if self
                .reference_projections
                .insert(token, (entity, origin))
                .is_some_and(|(existing, _)| existing != entity)
            {
                return Err(self.internal_token(token));
            }
        }
        self.references.insert(node, id);
        Ok(id)
    }

    fn constant_type(&self, id: ConstantId) -> Result<ScalarType, HeaderDefinitionError> {
        let source = self
            .sources
            .get(&id)
            .ok_or_else(|| self.rule(DefinitionRule::NonConstantExpression, id))?;
        self.scalar_type(source.ty, &mut HashSet::new())
            .ok_or_else(|| {
                Self::rule_at(
                    DefinitionRule::InvalidConstantType,
                    SyntaxOrigin::Node(source.initializer),
                )
            })
    }

    fn scalar_type_node(
        &self,
        ty_node: NodeId,
        expression: NodeId,
    ) -> Result<ScalarType, HeaderDefinitionError> {
        let bound = self
            .bindings
            .roots
            .get(&ty_node)
            .copied()
            .ok_or(HeaderDefinitionError::MissingType(ty_node))?;
        self.scalar_type(bound, &mut HashSet::new())
            .ok_or_else(|| Self::non_constant(expression))
    }

    pub(super) fn ensure_acyclic(&self) -> Result<(), HeaderDefinitionError> {
        let mut active = HashSet::new();
        let mut complete = HashSet::new();
        let mut ids = self.sources.keys().copied().collect::<Vec<_>>();
        ids.sort_unstable();
        for id in ids {
            self.visit_dependencies(id, &mut active, &mut complete)?;
        }
        Ok(())
    }

    fn visit_dependencies(
        &self,
        id: ConstantId,
        active: &mut HashSet<ConstantId>,
        complete: &mut HashSet<ConstantId>,
    ) -> Result<(), HeaderDefinitionError> {
        if complete.contains(&id) {
            return Ok(());
        }
        active.insert(id);
        for (dependency, origin) in self.dependencies.get(&id).map_or(&[][..], Vec::as_slice) {
            if active.contains(dependency) {
                return Err(Self::rule_at(
                    DefinitionRule::ConstantCycle,
                    SyntaxOrigin::Node(*origin),
                ));
            }
            self.visit_dependencies(*dependency, active, complete)?;
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }
}
