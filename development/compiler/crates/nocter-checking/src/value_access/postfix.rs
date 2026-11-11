//! Prepare a postfix spine iteratively; operands retain the ordinary source-order boundaries.
use super::{Failure, Rule, checker::Checker};
use nocter_model::AccessExpressionId;
use nocter_syntax::{NodeId, NodeKind};

impl Checker<'_, '_> {
    pub(super) fn postfix_value(
        &mut self,
        node: NodeId,
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
        let first = spine.iter().position(|&(_, _, suffix)| {
            suffix.is_none_or(|suffix| !matches!(self.kind(suffix), Ok(NodeKind::MemberSuffix)))
        });
        let Some(first) = first else {
            return self.member_value(node, reachable);
        };
        // Keep a static member prefix as one selected place. Its siblings must not acquire
        // a complete-parent read. A leading authored direct call also stays a direct call.
        let (head, base, suffix) = spine[first];
        let initial_call =
            suffix.is_some_and(|suffix| matches!(self.kind(suffix), Ok(NodeKind::CallSuffix)));
        let mut value = if initial_call {
            self.call(head, reachable)?
        } else {
            self.expression(base, None, reachable)?
        };
        let mut pending = spine
            .into_iter()
            .skip(first + usize::from(initial_call))
            .peekable();
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
                    while pending.peek().is_some_and(|(_, _, suffix)| {
                        suffix.is_some_and(|suffix| {
                            matches!(self.kind(suffix), Ok(NodeKind::MemberSuffix))
                        })
                    }) {
                        let (node, _, suffix) =
                            pending.next().ok_or(Failure::InconsistentSyntax)?;
                        end = node;
                        members
                            .push(self.member_token(suffix.ok_or(Failure::InconsistentSyntax)?)?);
                    }
                    self.component_value(end, value, &members)?
                }
                NodeKind::CallSuffix => self.call_value(step, suffix, value, reachable)?,
                _ => return Err(Failure::InconsistentSyntax),
            };
        }
        Ok(value)
    }
}
