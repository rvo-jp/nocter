//! Prepare a postfix spine iteratively; operands retain the ordinary source-order boundaries.
use super::{Failure, Rule, checker::Checker};
use nocter_model::AccessExpressionId;
use nocter_syntax::{NodeId, NodeKind};

impl Checker<'_, '_> {
    pub(super) fn postfix_value(
        &mut self,
        node: NodeId,
        projection: Option<super::ir::Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let mut spine = Vec::new();
        let mut root = self.unwrap(node)?;
        loop {
            if self.kind(root)? == NodeKind::TransferExpression {
                let base = self.first_child(root)?;
                spine.push((root, base, None));
                root = self.unwrap(base)?;
                continue;
            }
            if self.kind(root)? != NodeKind::PostfixExpression {
                break;
            }
            let children = self.children(root);
            let [base, suffix] = children.as_slice() else {
                return Err(Failure::InconsistentSyntax);
            };
            if !matches!(
                self.kind(*suffix)?,
                NodeKind::IndexSuffix | NodeKind::MemberSuffix | NodeKind::CallSuffix
            ) {
                return Err(Failure::InconsistentSyntax);
            }
            spine.push((root, *base, Some(*suffix)));
            root = self.unwrap(*base)?;
        }
        spine.reverse();
        // Keep a static member prefix as one selected place. Its siblings must not acquire
        // a complete-parent read. The first invocation chooses its input boundary before
        // composing the remaining postfix operations on the produced value.
        let first_call = spine.iter().position(|&(_, _, suffix)| {
            suffix.is_some_and(|suffix| matches!(self.kind(suffix), Ok(NodeKind::CallSuffix)))
        });
        let (mut value, next) = if let Some(first) = first_call {
            (self.call(spine[first].0, reachable)?, first + 1)
        } else {
            let first = spine.iter().position(|&(_, _, suffix)| {
                suffix.is_none_or(|suffix| !matches!(self.kind(suffix), Ok(NodeKind::MemberSuffix)))
            });
            let Some(first) = first else {
                return self.member_value(node, projection, reachable);
            };
            let projection =
                projection.filter(|_| spine.iter().all(|(_, _, suffix)| suffix.is_some()));
            let value =
                self.expression_with_projection(spine[first].1, None, None, projection, reachable)?;
            (value, first)
        };
        let mut pending = spine.into_iter().skip(next).peekable();
        while let Some((step, _, suffix)) = pending.next() {
            let Some(suffix) = suffix else {
                let Some(referent) = self.expression_referent(value) else {
                    return self.reject(Rule::MissingTransfer, step);
                };
                value = self.transfer_referent(step, referent)?;
                continue;
            };
            value = match self.kind(suffix)? {
                NodeKind::IndexSuffix => self.index_component(step, suffix, value, reachable)?,
                NodeKind::MemberSuffix => {
                    let mut members = vec![self.member_token(suffix)?];
                    let mut end = step;
                    while let Some(&(next, _, Some(next_suffix))) = pending.peek()
                        && self.kind(next_suffix)? == NodeKind::MemberSuffix
                    {
                        pending.next();
                        end = next;
                        members.push(self.member_token(next_suffix)?);
                    }
                    if let Some(&(call, _, Some(arguments))) = pending.peek()
                        && self.kind(arguments)? == NodeKind::CallSuffix
                    {
                        pending.next();
                        members.pop();
                        if !members.is_empty() {
                            value =
                                self.component_value(self.first_child(end)?, value, &members)?;
                        }
                        self.call_member_value(call, arguments, end, value, reachable)?
                    } else {
                        self.component_value(end, value, &members)?
                    }
                }
                NodeKind::CallSuffix => self.call_value(step, suffix, value, reachable)?,
                _ => return Err(Failure::InconsistentSyntax),
            };
        }
        Ok(value)
    }
}
