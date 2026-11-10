//! Contextual integer recipes carry data constraints, never a second source evaluator.
use super::{
    Binary, Failure,
    booleans::{self, Demand},
    checker::Checker,
};
use nocter_model::{AccessExpressionId, BuiltinType, TypeId, ValueAccess};
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
        Ok(
            if !logical && hint.is_none() && self.integer_recipe(left_node)? {
                // The left recipe has no names, calls or control. Obtain its constraint without
                // changing the frozen left-to-right execution/snapshot order.
                let right = self.expression(right_node, None, reachable)?;
                let data = self.integer_hint(self.ty(right).access().map(ValueAccess::data));
                let left = self.expression_with_context(left_node, data, None, reachable)?;
                (left, right, Demand::Always)
            } else {
                let left = self.expression_with_context(left_node, hint, None, reachable)?;
                let demand = booleans::right_demand(operation, self.normal_boolean(left));
                let data = if logical {
                    None
                } else {
                    self.integer_hint(self.ty(left).access().map(ValueAccess::data))
                        .or(hint)
                };
                let right = self.expression_with_context(
                    right_node,
                    data,
                    None,
                    reachable && self.ty(left).may_complete() && demand != Demand::Never,
                )?;
                (left, right, demand)
            },
        )
    }
    pub fn integer_data(&self, data: TypeId) -> bool {
        matches!(
            self.program().data_kind(data),
            Some(BuiltinType::I32 | BuiltinType::Usize)
        )
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
