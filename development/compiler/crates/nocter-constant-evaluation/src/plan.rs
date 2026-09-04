use std::collections::HashMap;

use nocter_model::{BuiltinType, ConstantValue};
use nocter_source::SourceFile;
use nocter_syntax::SyntaxOrigin;
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxTree, TokenKind, decode_character_literal,
    decode_plain_string_expression, direct_node, first_direct_token,
};

use crate::model::{
    ConstantExpressionPlan, ConstantOperation, ConstantPlanError, ConstantPlanRule,
    ConstantReference, ConstantResolver, ConstantScalarType, FrozenExpressionPlan, FrozenType,
    PlanNode, PlanNodeId,
};
use crate::support::{
    direct_punctuation, expression_children, integer_spec, one_expression_child, parse_integer,
};

struct Planner<'a, R> {
    source: &'a SourceFile,
    tree: &'a SyntaxTree,
    resolver: &'a mut R,
    types: HashMap<NodeId, ConstantScalarType>,
    references: HashMap<NodeId, ConstantReference>,
    conversion_types: HashMap<NodeId, Option<ConstantScalarType>>,
    nodes: Vec<PlanNode>,
}

/// Produces one closed typed plan using the caller-owned name and type authority.
///
/// Both sides of a short-circuit expression are planned. Evaluation may skip the right value, but
/// malformed syntax, type errors, and dependency cycles therefore remain observable.
///
/// # Errors
///
/// Returns an authored constant-expression rule, a caller resolver failure, or an invalid syntax
/// identity when the supplied source, tree, and expression do not describe one coherent input.
pub fn plan_expression<R: ConstantResolver>(
    source: &SourceFile,
    tree: &SyntaxTree,
    expression: NodeId,
    expected: ConstantScalarType,
    resolver: &mut R,
) -> Result<ConstantExpressionPlan, ConstantPlanError<R::Error>> {
    let mut planner = Planner {
        source,
        tree,
        resolver,
        types: HashMap::new(),
        references: HashMap::new(),
        conversion_types: HashMap::new(),
        nodes: Vec::new(),
    };
    planner.analyze(expression, Some(expected))?;
    let root = planner.build(expression)?;
    Ok(ConstantExpressionPlan {
        nodes: planner.nodes,
        root,
    })
}

/// Produces one closed plan for a scalar or recursively fixed-array static initializer.
///
/// Aggregate structure is validated here while every scalar leaf goes through [`plan_expression`],
/// preserving one authority for compile-time arithmetic, conversions, and constant references.
///
/// # Errors
///
/// Returns the same authored and caller-context failures as scalar planning. A wrong aggregate
/// shape is reported as a type mismatch at the initializer node.
pub fn plan_frozen_expression<R: ConstantResolver>(
    source: &SourceFile,
    tree: &SyntaxTree,
    expression: NodeId,
    expected: &FrozenType,
    resolver: &mut R,
) -> Result<FrozenExpressionPlan, ConstantPlanError<R::Error>> {
    let semantic =
        unwrap_expression(tree, expression).ok_or(ConstantPlanError::InvalidSyntax(expression))?;
    match expected {
        FrozenType::Scalar(expected) => {
            plan_expression(source, tree, expression, *expected, resolver)
                .map(FrozenExpressionPlan::Scalar)
        }
        FrozenType::FixedArray { element, length } => {
            if tree.node(semantic).map(nocter_syntax::SyntaxNode::kind)
                != Some(NodeKind::ArrayLiteral)
            {
                return Err(ConstantPlanError::Rule {
                    rule: ConstantPlanRule::TypeMismatch,
                    origin: SyntaxOrigin::Node(expression),
                });
            }
            let children = expression_children(tree, semantic);
            if children.len() != *length {
                return Err(ConstantPlanError::Rule {
                    rule: ConstantPlanRule::TypeMismatch,
                    origin: SyntaxOrigin::Node(expression),
                });
            }
            let elements = children
                .into_iter()
                .map(|child| plan_frozen_expression(source, tree, child, element, resolver))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            Ok(FrozenExpressionPlan::FixedArray {
                ty: expected.clone(),
                elements,
            })
        }
    }
}

fn unwrap_expression(tree: &SyntaxTree, mut node: NodeId) -> Option<NodeId> {
    loop {
        match tree.node(node)?.kind() {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                node = one_expression_child(tree, node)?;
            }
            _ => return Some(node),
        }
    }
}

impl<R: ConstantResolver> Planner<'_, R> {
    #[allow(clippy::too_many_lines)] // Exhaustive syntax-to-plan type contract.
    fn analyze(
        &mut self,
        node: NodeId,
        expected: Option<ConstantScalarType>,
    ) -> Result<ConstantScalarType, ConstantPlanError<R::Error>> {
        let kind = self.kind(node)?;
        let ty = match kind {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child = one_expression_child(self.tree, node)
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                self.analyze(child, expected)?
            }
            NodeKind::ScalarLiteral => {
                let token = first_direct_token(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => ConstantScalarType::Bool,
                    TokenKind::CharacterLiteral => ConstantScalarType::Character,
                    TokenKind::IntegerLiteral => expected
                        .filter(|ty| matches!(ty, ConstantScalarType::Integer(_)))
                        .unwrap_or(ConstantScalarType::Integer(BuiltinType::I32)),
                    _ => {
                        return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
                    }
                }
            }
            NodeKind::StringExpression => ConstantScalarType::Text,
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                let reference = self.reference(node)?;
                reference.ty()
            }
            NodeKind::UnaryExpression => {
                let operator = direct_punctuation(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let operand = one_expression_child(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                match operator {
                    Punctuation::Bang => {
                        self.analyze(operand, Some(ConstantScalarType::Bool))?;
                        ConstantScalarType::Bool
                    }
                    Punctuation::Minus => {
                        let ty = expected
                            .or(self.scalar_hint(operand)?)
                            .unwrap_or(ConstantScalarType::Integer(BuiltinType::I32));
                        if !matches!(ty, ConstantScalarType::Integer(builtin) if integer_spec(builtin).is_some_and(|spec| spec.signed))
                        {
                            return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                        }
                        self.analyze(operand, Some(ty))?;
                        ty
                    }
                    _ => {
                        return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
                    }
                }
            }
            NodeKind::LogicalAndExpression | NodeKind::LogicalOrExpression => {
                let operands = self.binary_operands(node)?;
                self.analyze(operands[0], Some(ConstantScalarType::Bool))?;
                self.analyze(operands[1], Some(ConstantScalarType::Bool))?;
                ConstantScalarType::Bool
            }
            NodeKind::EqualityExpression | NodeKind::OrderingExpression => {
                let operands = self.binary_operands(node)?;
                let operand_ty = self
                    .merge_hints(operands[0], operands[1], node)?
                    .unwrap_or(ConstantScalarType::Integer(BuiltinType::I32));
                let left = self.analyze(operands[0], Some(operand_ty))?;
                let right = self.analyze(operands[1], Some(left))?;
                if left != right
                    || kind == NodeKind::OrderingExpression
                        && !matches!(
                            left,
                            ConstantScalarType::Integer(_) | ConstantScalarType::Character
                        )
                {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                ConstantScalarType::Bool
            }
            NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                let ty = expected
                    .or(self.merge_hints(operands[0], operands[1], node)?)
                    .unwrap_or(ConstantScalarType::Integer(BuiltinType::I32));
                if !matches!(ty, ConstantScalarType::Integer(_)) {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                self.analyze(operands[0], Some(ty))?;
                self.analyze(operands[1], Some(ty))?;
                ty
            }
            NodeKind::ConversionExpression => {
                let operand = one_expression_child(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let ty_node = direct_node(self.tree, node, NodeKind::Type)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let target = self
                    .conversion_type(ty_node)?
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                if !matches!(target, ConstantScalarType::Integer(_)) {
                    return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
                }
                let operand_ty = self.analyze(operand, None)?;
                if !matches!(operand_ty, ConstantScalarType::Integer(_)) {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                target
            }
            _ => return Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
        };
        if expected.is_some_and(|expected| expected != ty) {
            return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
        }
        self.types.insert(node, ty);
        Ok(ty)
    }

    fn scalar_hint(
        &mut self,
        node: NodeId,
    ) -> Result<Option<ConstantScalarType>, ConstantPlanError<R::Error>> {
        match self.kind(node)? {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child = one_expression_child(self.tree, node)
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                self.scalar_hint(child)
            }
            NodeKind::ScalarLiteral => {
                let token = first_direct_token(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                Ok(match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => {
                        Some(ConstantScalarType::Bool)
                    }
                    TokenKind::CharacterLiteral => Some(ConstantScalarType::Character),
                    TokenKind::IntegerLiteral => None,
                    _ => {
                        return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
                    }
                })
            }
            NodeKind::StringExpression => Ok(Some(ConstantScalarType::Text)),
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                self.reference(node).map(|reference| Some(reference.ty()))
            }
            NodeKind::UnaryExpression => {
                let operator = direct_punctuation(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                if operator == Punctuation::Bang {
                    Ok(Some(ConstantScalarType::Bool))
                } else {
                    let operand = one_expression_child(self.tree, node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                    self.scalar_hint(operand)
                }
            }
            NodeKind::LogicalAndExpression
            | NodeKind::LogicalOrExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression => Ok(Some(ConstantScalarType::Bool)),
            NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                self.merge_hints(operands[0], operands[1], node)
            }
            NodeKind::ConversionExpression => {
                let ty_node = direct_node(self.tree, node, NodeKind::Type)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                self.conversion_type(ty_node)
            }
            _ => Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
        }
    }

    fn merge_hints(
        &mut self,
        left: NodeId,
        right: NodeId,
        origin: NodeId,
    ) -> Result<Option<ConstantScalarType>, ConstantPlanError<R::Error>> {
        let left = self.scalar_hint(left)?;
        let right = self.scalar_hint(right)?;
        match (left, right) {
            (Some(left), Some(right)) if left != right => {
                Err(self.rule(ConstantPlanRule::TypeMismatch, origin))
            }
            (Some(ty), _) | (_, Some(ty)) => Ok(Some(ty)),
            (None, None) => Ok(None),
        }
    }

    fn build(&mut self, node: NodeId) -> Result<PlanNodeId, ConstantPlanError<R::Error>> {
        let ty = self
            .types
            .get(&node)
            .copied()
            .ok_or(ConstantPlanError::InvalidSyntax(node))?;
        let kind = self.kind(node)?;
        let operation = match kind {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                return self.build(
                    one_expression_child(self.tree, node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                );
            }
            NodeKind::ScalarLiteral => {
                let token = first_direct_token(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => ConstantOperation::Value(
                        ConstantValue::Bool(token.kind() == TokenKind::Keyword(Keyword::True)),
                    ),
                    TokenKind::CharacterLiteral => {
                        ConstantOperation::Value(ConstantValue::Character(
                            decode_character_literal(
                                self.source
                                    .text_at(token.range())
                                    .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                            )
                            .ok_or_else(|| {
                                self.rule(ConstantPlanRule::NonConstantExpression, node)
                            })?,
                        ))
                    }
                    TokenKind::IntegerLiteral => ConstantOperation::IntegerLiteral(
                        parse_integer(
                            self.source
                                .text_at(token.range())
                                .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                        )
                        .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?,
                    ),
                    _ => return Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
                }
            }
            NodeKind::StringExpression => ConstantOperation::Value(ConstantValue::Text(
                decode_plain_string_expression(self.source, self.tree, node)
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?,
            )),
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => {
                ConstantOperation::Reference(
                    self.references
                        .get(&node)
                        .copied()
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?
                        .id(),
                )
            }
            NodeKind::UnaryExpression => ConstantOperation::Unary {
                operator: direct_punctuation(self.tree, node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                operand: self.build(
                    one_expression_child(self.tree, node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                )?,
            },
            NodeKind::LogicalAndExpression
            | NodeKind::LogicalOrExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression
            | NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                ConstantOperation::Binary {
                    operator: direct_punctuation(self.tree, node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                    left: self.build(operands[0])?,
                    right: self.build(operands[1])?,
                }
            }
            NodeKind::ConversionExpression => ConstantOperation::Conversion {
                operand: self.build(
                    one_expression_child(self.tree, node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                )?,
            },
            _ => return Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
        };
        let id = PlanNodeId(self.nodes.len());
        self.nodes.push(PlanNode {
            ty,
            origin: SyntaxOrigin::Node(node),
            operation,
        });
        Ok(id)
    }

    fn reference(
        &mut self,
        node: NodeId,
    ) -> Result<ConstantReference, ConstantPlanError<R::Error>> {
        if let Some(reference) = self.references.get(&node) {
            return Ok(*reference);
        }
        let reference = self
            .resolver
            .resolve_constant(node)
            .map_err(ConstantPlanError::Context)?;
        self.references.insert(node, reference);
        Ok(reference)
    }

    fn conversion_type(
        &mut self,
        node: NodeId,
    ) -> Result<Option<ConstantScalarType>, ConstantPlanError<R::Error>> {
        if let Some(ty) = self.conversion_types.get(&node) {
            return Ok(*ty);
        }
        let ty = self
            .resolver
            .resolve_type(node)
            .map_err(ConstantPlanError::Context)?;
        self.conversion_types.insert(node, ty);
        Ok(ty)
    }

    fn binary_operands(&self, node: NodeId) -> Result<[NodeId; 2], ConstantPlanError<R::Error>> {
        expression_children(self.tree, node)
            .try_into()
            .map_err(|_| ConstantPlanError::InvalidSyntax(node))
    }

    fn kind(&self, node: NodeId) -> Result<NodeKind, ConstantPlanError<R::Error>> {
        self.tree
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(ConstantPlanError::InvalidSyntax(node))
    }

    fn rule(&self, rule: ConstantPlanRule, node: NodeId) -> ConstantPlanError<R::Error> {
        debug_assert_eq!(node.source(), self.tree.source());
        ConstantPlanError::Rule {
            rule,
            origin: SyntaxOrigin::Node(node),
        }
    }
}
