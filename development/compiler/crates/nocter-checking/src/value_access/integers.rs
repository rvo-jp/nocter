//! Contextual integer recipes carry data constraints, never a second source evaluator.
use super::{
    Binary, Failure,
    booleans::{self, Demand},
    checker::Checker,
    ir::Projection,
};
use nocter_model::{AccessExpressionId, TypeId, ValueAccess, ValueAccessMode};
use nocter_syntax::{NodeId, NodeKind, Punctuation, TokenKind};
impl Checker<'_, '_> {
    pub fn binary_operands(
        &mut self,
        left_node: NodeId,
        right_node: NodeId,
        operation: Binary,
        hint: Option<TypeId>,
        reachable: bool,
    ) -> Result<(AccessExpressionId, AccessExpressionId, Demand), Failure> {
        let logical = matches!(operation, Binary::And | Binary::Or);
        let projection = operation
            .is_comparison()
            .then_some(Projection::input(ValueAccessMode::Read));
        Ok(
            if !logical && hint.is_none() && self.integer_recipe(left_node)? {
                // The left recipe has no names, calls or control. Obtain its constraint without
                // changing the frozen left-to-right execution/snapshot order.
                let right =
                    self.expression_with_type_hint(right_node, None, projection, reachable)?;
                let data = self.integer_hint(self.ty(right).access().map(ValueAccess::data));
                let left =
                    self.expression_with_type_hint(left_node, data, projection, reachable)?;
                (left, right, Demand::Always)
            } else {
                let left =
                    self.expression_with_type_hint(left_node, hint, projection, reachable)?;
                let demand = booleans::right_demand(operation, self.normal_boolean(left));
                let data = if logical {
                    None
                } else {
                    self.integer_hint(self.ty(left).access().map(ValueAccess::data))
                        .or(hint)
                };
                let right = self.expression_with_type_hint(
                    right_node,
                    data,
                    projection,
                    reachable && self.ty(left).may_complete() && demand != Demand::Never,
                )?;
                (left, right, demand)
            },
        )
    }
    pub fn integer_data(&self, data: TypeId) -> bool {
        self.program()
            .data_kind(data)
            .and_then(nocter_model::BuiltinNumericType::for_builtin)
            .and_then(nocter_model::BuiltinNumericType::integer_bounds)
            .is_some()
    }
    pub fn integer_hint(&self, hint: Option<TypeId>) -> Option<TypeId> {
        hint.filter(|&data| self.integer_data(data))
    }
    /// Only syntax made entirely from contextual integer literals can receive another operand's
    /// data constraint. Typed names/calls never become literals because of their numerical value.
    pub fn integer_recipe(&mut self, node: NodeId) -> Result<bool, Failure> {
        let root = self.unwrap(node)?;
        let mut pending = vec![(root, false)];
        while let Some((node, expanded)) = pending.pop() {
            let node = self.unwrap(node)?;
            if self.integer_recipes.contains_key(&node) {
                continue;
            }
            let children = match self.kind(node)? {
                NodeKind::ScalarLiteral => {
                    self.integer_recipes
                        .insert(node, self.token(node)?.kind() == TokenKind::IntegerLiteral);
                    continue;
                }
                NodeKind::UnaryExpression if self.has(node, Punctuation::Minus) => {
                    self.children(node)
                }
                NodeKind::AdditiveExpression | NodeKind::MultiplicativeExpression => {
                    self.children(node)
                }
                _ => {
                    self.integer_recipes.insert(node, false);
                    continue;
                }
            };
            if expanded {
                let flexible = children
                    .into_iter()
                    .map(|child| self.unwrap(child))
                    .collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .all(|child| self.integer_recipes.get(child) == Some(&true));
                self.integer_recipes.insert(node, flexible);
            } else {
                pending.push((node, true));
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
            }
        }
        self.integer_recipes
            .get(&root)
            .copied()
            .ok_or(Failure::InconsistentSyntax)
    }
}

impl Checker<'_, '_> {
    /// A normal-result singleton is not permission to skip evaluation or read a variable early.
    /// No arithmetic evaluator or callee-body inference is introduced here.
    pub(super) fn integer_result(&self, kind: &super::ExpressionKind) -> Option<i128> {
        use super::ExpressionKind;
        match *kind {
            ExpressionKind::Integer(value) => Some(value),
            ExpressionKind::Boundary { source, .. } => {
                self.expressions.get(source)?.normal_integer()
            }
            ExpressionKind::If {
                condition,
                then_block,
                else_block,
            } => {
                let result = |id| {
                    let block = self.blocks.get(id)?;
                    block
                        .ty
                        .may_complete()
                        .then_some(block.result?)
                        .and_then(|id| self.expressions.get(id)?.normal_integer())
                };
                match self.normal_boolean(condition) {
                    Some(true) => result(then_block),
                    Some(false) => result(else_block),
                    None => {
                        let left = self.blocks.get(then_block)?;
                        let right = self.blocks.get(else_block)?;
                        match (left.ty.may_complete(), right.ty.may_complete()) {
                            (true, false) => result(then_block),
                            (false, true) => result(else_block),
                            (true, true) => {
                                result(then_block).filter(|left| Some(*left) == result(else_block))
                            }
                            _ => None,
                        }
                    }
                }
            }
            _ => None,
        }
    }
}
