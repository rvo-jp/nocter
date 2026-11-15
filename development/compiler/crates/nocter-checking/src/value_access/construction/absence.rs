//! Context-only absence construction inference over shared syntax; no runtime tag or owner proof.
use crate::value_access::{Failure, checker::Checker};
use nocter_syntax::NodeId;

impl Checker<'_, '_> {
    fn literal_absence(&self, node: NodeId) -> Result<bool, Failure> {
        let node = self.unwrap(node)?;
        Ok(self.kind(node)? == nocter_syntax::NodeKind::ScalarLiteral
            && self.token(node)?.kind()
                == nocter_syntax::TokenKind::Keyword(nocter_syntax::Keyword::None))
    }
    /// A memoized syntax-only construction request, never a runtime tag or value proof.
    pub(in crate::value_access) fn absence_construction(
        &mut self,
        node: NodeId,
    ) -> Result<bool, Failure> {
        let root = self.unwrap(node)?;
        let mut pending = vec![(root, false)];
        while let Some((node, expanded)) = pending.pop() {
            if self.absence_recipes.contains_key(&node) {
                continue;
            }
            let Some(children) = self.absence_dependencies(node)? else {
                self.absence_recipes.insert(node, false);
                continue;
            };
            let children = children
                .into_iter()
                .map(|child| self.unwrap(child))
                .collect::<Result<Vec<_>, _>>()?;
            if expanded || children.is_empty() {
                let absent = children
                    .iter()
                    .all(|child| self.absence_recipes.get(child) == Some(&true));
                self.absence_recipes.insert(node, absent);
            } else {
                pending.push((node, true));
                pending.extend(children.into_iter().map(|child| (child, false)));
            }
        }
        self.absence_recipes
            .get(&root)
            .copied()
            .ok_or(Failure::InconsistentSyntax)
    }
    fn absence_dependencies(&self, node: NodeId) -> Result<Option<Vec<NodeId>>, Failure> {
        use nocter_syntax::NodeKind;
        if self.literal_absence(node)? {
            return Ok(Some(Vec::new()));
        }
        match self.kind(node)? {
            NodeKind::IfExpression => {
                let Ok(alternative) = self.direct(node, NodeKind::ElseClause) else {
                    return Ok(None);
                };
                Ok(Some(vec![
                    self.direct(node, NodeKind::Block)?,
                    self.first_child(alternative)?,
                ]))
            }
            NodeKind::Block => {
                let Ok(sequence) = self.direct(node, NodeKind::ExecutableSequence) else {
                    return Ok(None);
                };
                let Ok(result) = self.direct(sequence, NodeKind::BodyResult) else {
                    return Ok(None);
                };
                Ok(Some(vec![self.first_child(result)?]))
            }
            _ => Ok(None),
        }
    }
}
