use std::collections::HashMap;

use nocter_model::{
    BuiltinType, CompilationTarget, ConstantValue, lossless_builtin_numeric_conversion,
};
use nocter_syntax::{BoundSyntax, FloatLiteralSpelling, SyntaxOrigin};
use nocter_syntax::{
    Keyword, NodeId, NodeKind, Punctuation, SyntaxTree, TokenKind, decode_character_literal,
    decode_plain_string_expression, direct_node, first_direct_token,
};

use crate::FloatFormat;
use crate::model::{
    ConstantExpressionPlan, ConstantOperation, ConstantPlanError, ConstantPlanRule,
    ConstantReference, ConstantResolver, ConstantScalarType, FrozenExpressionPlan, FrozenType,
    PlanNode, PlanNodeId,
};
use crate::support::{
    direct_punctuation, expression_children, integer_spec, one_expression_child, parse_integer,
};

struct Planner<'a, R> {
    syntax: BoundSyntax<'a>,
    resolver: &'a mut R,
    types: HashMap<NodeId, ConstantScalarType>,
    references: HashMap<NodeId, ConstantReference>,
    conversion_types: HashMap<NodeId, Option<ConstantScalarType>>,
    nodes: Vec<PlanNode>,
}

#[derive(Clone, Copy)]
enum NumericDefault {
    Integer,
    Float,
}

#[derive(Clone, Copy)]
struct ScalarHint {
    exact: Option<ConstantScalarType>,
    numeric_default: NumericDefault,
}

impl ScalarHint {
    const fn exact(ty: ConstantScalarType) -> Self {
        Self {
            exact: Some(ty),
            numeric_default: if matches!(ty, ConstantScalarType::Float(_)) {
                NumericDefault::Float
            } else {
                NumericDefault::Integer
            },
        }
    }

    const fn flexible_integer() -> Self {
        Self {
            exact: None,
            numeric_default: NumericDefault::Integer,
        }
    }

    const fn flexible_float() -> Self {
        Self {
            exact: None,
            numeric_default: NumericDefault::Float,
        }
    }

    const fn inferred_or_default(self) -> ConstantScalarType {
        match (self.exact, self.numeric_default) {
            (Some(ty), _) => ty,
            (None, NumericDefault::Integer) => ConstantScalarType::Integer(BuiltinType::I32),
            (None, NumericDefault::Float) => ConstantScalarType::Float(FloatFormat::Binary64),
        }
    }
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
    target: CompilationTarget,
    syntax: BoundSyntax<'_>,
    expression: NodeId,
    expected: ConstantScalarType,
    resolver: &mut R,
) -> Result<ConstantExpressionPlan, ConstantPlanError<R::Error>> {
    let tree = syntax.tree();
    if tree.node(expression).is_none() {
        return Err(ConstantPlanError::InvalidSyntax(expression));
    }
    let mut planner = Planner {
        syntax,
        resolver,
        types: HashMap::new(),
        references: HashMap::new(),
        conversion_types: HashMap::new(),
        nodes: Vec::new(),
    };
    planner.analyze(expression, Some(expected))?;
    let root = planner.build(expression)?;
    Ok(ConstantExpressionPlan {
        target,
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
    target: CompilationTarget,
    syntax: BoundSyntax<'_>,
    expression: NodeId,
    expected: &FrozenType,
    resolver: &mut R,
) -> Result<FrozenExpressionPlan, ConstantPlanError<R::Error>> {
    let tree = syntax.tree();
    let semantic =
        unwrap_expression(tree, expression).ok_or(ConstantPlanError::InvalidSyntax(expression))?;
    match expected {
        FrozenType::Scalar(expected) => {
            plan_expression(target, syntax, expression, *expected, resolver)
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
                .map(|child| plan_frozen_expression(target, syntax, child, element, resolver))
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
                let child = one_expression_child(self.syntax.tree(), node)
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                self.analyze(child, expected)?
            }
            NodeKind::ScalarLiteral => {
                let token = first_direct_token(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => ConstantScalarType::Bool,
                    TokenKind::CharacterLiteral => ConstantScalarType::Character,
                    TokenKind::IntegerLiteral => expected
                        .filter(|ty| matches!(ty, ConstantScalarType::Integer(_)))
                        .unwrap_or(ConstantScalarType::Integer(BuiltinType::I32)),
                    TokenKind::FloatLiteral => {
                        let authored = self
                            .syntax
                            .source()
                            .text_at(token.range())
                            .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                        let suffix = FloatLiteralSpelling::from_authored(authored)
                            .suffix()
                            .map(FloatFormat::from);
                        suffix
                            .map(ConstantScalarType::Float)
                            .or_else(|| {
                                expected.filter(|ty| matches!(ty, ConstantScalarType::Float(_)))
                            })
                            .unwrap_or(ConstantScalarType::Float(FloatFormat::Binary64))
                    }
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
                let operator = direct_punctuation(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let operand = one_expression_child(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                match operator {
                    Punctuation::Bang => {
                        self.analyze(operand, Some(ConstantScalarType::Bool))?;
                        ConstantScalarType::Bool
                    }
                    Punctuation::Minus => {
                        let ty = match expected {
                            Some(expected) => expected,
                            None => self.scalar_hint(operand)?.inferred_or_default(),
                        };
                        if !matches!(
                            ty,
                            ConstantScalarType::Integer(builtin)
                                if integer_spec(builtin).is_some_and(|spec| spec.signed)
                        ) && !matches!(ty, ConstantScalarType::Float(_))
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
                    .inferred_or_default();
                let left = self.analyze(operands[0], Some(operand_ty))?;
                let right = self.analyze(operands[1], Some(left))?;
                if left != right
                    || kind == NodeKind::OrderingExpression
                        && !matches!(
                            left,
                            ConstantScalarType::Integer(_)
                                | ConstantScalarType::Character
                                | ConstantScalarType::Float(_)
                        )
                {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                ConstantScalarType::Bool
            }
            NodeKind::ShiftExpression => {
                let operands = self.binary_operands(node)?;
                let hint = self.merge_hints(operands[0], operands[1], node)?;
                let ty = expected.unwrap_or_else(|| hint.inferred_or_default());
                if !matches!(ty, ConstantScalarType::Integer(_)) {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                self.analyze(operands[0], Some(ty))?;
                self.analyze(operands[1], Some(ty))?;
                ty
            }
            NodeKind::AdditiveExpression | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                let hint = self.merge_hints(operands[0], operands[1], node)?;
                let ty = expected.unwrap_or_else(|| hint.inferred_or_default());
                if !matches!(
                    ty,
                    ConstantScalarType::Integer(_) | ConstantScalarType::Float(_)
                ) {
                    return Err(self.rule(ConstantPlanRule::TypeMismatch, node));
                }
                self.analyze(operands[0], Some(ty))?;
                self.analyze(operands[1], Some(ty))?;
                ty
            }
            NodeKind::ConversionExpression => {
                let operand = one_expression_child(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let ty_node = direct_node(self.syntax.tree(), node, NodeKind::Type)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                let target = self
                    .conversion_type(ty_node)?
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                let operand_ty = self.analyze(operand, None)?;
                let valid = match (operand_ty, target) {
                    (ConstantScalarType::Integer(_), ConstantScalarType::Integer(_)) => true,
                    (source, target) => match (numeric_builtin(source), numeric_builtin(target)) {
                        (Some(source), Some(target)) => {
                            lossless_builtin_numeric_conversion(source, target)
                        }
                        _ => false,
                    },
                };
                if !valid {
                    return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
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

    fn scalar_hint(&mut self, node: NodeId) -> Result<ScalarHint, ConstantPlanError<R::Error>> {
        match self.kind(node)? {
            NodeKind::Expression | NodeKind::GroupedExpression => {
                let child = one_expression_child(self.syntax.tree(), node)
                    .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node))?;
                self.scalar_hint(child)
            }
            NodeKind::ScalarLiteral => {
                let token = first_direct_token(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                Ok(match token.kind() {
                    TokenKind::Keyword(Keyword::True | Keyword::False) => {
                        ScalarHint::exact(ConstantScalarType::Bool)
                    }
                    TokenKind::CharacterLiteral => ScalarHint::exact(ConstantScalarType::Character),
                    TokenKind::IntegerLiteral => ScalarHint::flexible_integer(),
                    TokenKind::FloatLiteral => {
                        let authored = self
                            .syntax
                            .source()
                            .text_at(token.range())
                            .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                        let suffix = FloatLiteralSpelling::from_authored(authored)
                            .suffix()
                            .map(FloatFormat::from);
                        suffix.map_or_else(ScalarHint::flexible_float, |format| {
                            ScalarHint::exact(ConstantScalarType::Float(format))
                        })
                    }
                    _ => {
                        return Err(self.rule(ConstantPlanRule::NonConstantExpression, node));
                    }
                })
            }
            NodeKind::StringExpression => Ok(ScalarHint::exact(ConstantScalarType::Text)),
            NodeKind::ReferenceExpression | NodeKind::PostfixExpression => self
                .reference(node)
                .map(|reference| ScalarHint::exact(reference.ty())),
            NodeKind::UnaryExpression => {
                let operator = direct_punctuation(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                if operator == Punctuation::Bang {
                    Ok(ScalarHint::exact(ConstantScalarType::Bool))
                } else {
                    let operand = one_expression_child(self.syntax.tree(), node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                    self.scalar_hint(operand)
                }
            }
            NodeKind::LogicalAndExpression
            | NodeKind::LogicalOrExpression
            | NodeKind::EqualityExpression
            | NodeKind::OrderingExpression => Ok(ScalarHint::exact(ConstantScalarType::Bool)),
            NodeKind::ShiftExpression
            | NodeKind::AdditiveExpression
            | NodeKind::MultiplicativeExpression => {
                let operands = self.binary_operands(node)?;
                self.merge_hints(operands[0], operands[1], node)
            }
            NodeKind::ConversionExpression => {
                let ty_node = direct_node(self.syntax.tree(), node, NodeKind::Type)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?;
                self.conversion_type(ty_node)?.map_or_else(
                    || Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
                    |ty| Ok(ScalarHint::exact(ty)),
                )
            }
            _ => Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
        }
    }

    fn merge_hints(
        &mut self,
        left: NodeId,
        right: NodeId,
        origin: NodeId,
    ) -> Result<ScalarHint, ConstantPlanError<R::Error>> {
        let left = self.scalar_hint(left)?;
        let right = self.scalar_hint(right)?;
        let exact = match (left.exact, right.exact) {
            (Some(left), Some(right)) if left != right => {
                return Err(self.rule(ConstantPlanRule::TypeMismatch, origin));
            }
            (Some(ty), _) | (_, Some(ty)) => Some(ty),
            (None, None) => None,
        };
        let numeric_default = if matches!(left.numeric_default, NumericDefault::Float)
            || matches!(right.numeric_default, NumericDefault::Float)
        {
            NumericDefault::Float
        } else {
            NumericDefault::Integer
        };
        Ok(ScalarHint {
            exact,
            numeric_default,
        })
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
                    one_expression_child(self.syntax.tree(), node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                );
            }
            NodeKind::ScalarLiteral => self.build_scalar_literal(node)?,
            NodeKind::StringExpression => ConstantOperation::Value(ConstantValue::Text(
                decode_plain_string_expression(self.syntax, node)
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
                operator: direct_punctuation(self.syntax.tree(), node)
                    .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                operand: self.build(
                    one_expression_child(self.syntax.tree(), node)
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
                    operator: direct_punctuation(self.syntax.tree(), node)
                        .ok_or(ConstantPlanError::InvalidSyntax(node))?,
                    left: self.build(operands[0])?,
                    right: self.build(operands[1])?,
                }
            }
            NodeKind::ConversionExpression => ConstantOperation::Conversion {
                operand: self.build(
                    one_expression_child(self.syntax.tree(), node)
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

    fn build_scalar_literal(
        &self,
        node: NodeId,
    ) -> Result<ConstantOperation, ConstantPlanError<R::Error>> {
        let token = first_direct_token(self.syntax.tree(), node)
            .ok_or(ConstantPlanError::InvalidSyntax(node))?;
        let authored = || {
            self.syntax
                .source()
                .text_at(token.range())
                .ok_or(ConstantPlanError::InvalidSyntax(node))
        };
        match token.kind() {
            TokenKind::Keyword(Keyword::True | Keyword::False) => Ok(ConstantOperation::Value(
                ConstantValue::Bool(token.kind() == TokenKind::Keyword(Keyword::True)),
            )),
            TokenKind::CharacterLiteral => decode_character_literal(authored()?)
                .map(ConstantValue::Character)
                .map(ConstantOperation::Value)
                .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node)),
            TokenKind::IntegerLiteral => parse_integer(authored()?)
                .map(ConstantOperation::IntegerLiteral)
                .ok_or_else(|| self.rule(ConstantPlanRule::NonConstantExpression, node)),
            TokenKind::FloatLiteral => {
                let spelling = FloatLiteralSpelling::from_authored(authored()?);
                Ok(ConstantOperation::FloatLiteral(spelling.decimal().into()))
            }
            _ => Err(self.rule(ConstantPlanRule::NonConstantExpression, node)),
        }
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
        expression_children(self.syntax.tree(), node)
            .try_into()
            .map_err(|_| ConstantPlanError::InvalidSyntax(node))
    }

    fn kind(&self, node: NodeId) -> Result<NodeKind, ConstantPlanError<R::Error>> {
        self.syntax
            .tree()
            .node(node)
            .map(nocter_syntax::SyntaxNode::kind)
            .ok_or(ConstantPlanError::InvalidSyntax(node))
    }

    fn rule(&self, rule: ConstantPlanRule, node: NodeId) -> ConstantPlanError<R::Error> {
        debug_assert_eq!(node.source(), self.syntax.tree().source());
        ConstantPlanError::Rule {
            rule,
            origin: SyntaxOrigin::Node(node),
        }
    }
}

const fn numeric_builtin(ty: ConstantScalarType) -> Option<BuiltinType> {
    match ty {
        ConstantScalarType::Integer(builtin) => Some(builtin),
        ConstantScalarType::Float(FloatFormat::Binary32) => Some(BuiltinType::F32),
        ConstantScalarType::Float(FloatFormat::Binary64) => Some(BuiltinType::F64),
        ConstantScalarType::Bool | ConstantScalarType::Character | ConstantScalarType::Text => None,
    }
}
