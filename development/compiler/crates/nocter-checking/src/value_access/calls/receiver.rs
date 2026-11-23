//! A receiver's data selects the method before contextual access through outcomes.
use super::{
    AccessExpressionId, Callee, Checker, Failure, NodeId, NodeKind, PayloadSchema, SyntaxToken,
    TypeId,
};
use crate::value_access::ir::Projection;

#[derive(Clone, Copy)]
enum Step {
    Outcome(NodeId),
    Recovery(NodeId),
    Member { node: NodeId, name: SyntaxToken },
    Index { node: NodeId, suffix: NodeId },
}

pub(super) struct Receiver {
    root: AccessExpressionId,
    path: Vec<Step>,
}

impl Checker<'_, '_> {
    pub(super) fn prepare_member_callee(
        &mut self,
        selection: NodeId,
        base: NodeId,
        name: SyntaxToken,
        reachable: bool,
    ) -> Result<Callee, Failure> {
        let receiver = self.prepare_receiver(base, reachable)?;
        let member = match self.receiver_data(&receiver)? {
            Some(data) => self.member_for(data, name)?,
            None => None,
        };
        let projection = if let Some(member) = member {
            self.require_type_member(member, name, true)?;
            Some(self.receiver_projection(member.code)?)
        } else {
            None
        };
        let value = self.receiver_value(receiver, projection, reachable)?;
        if let Some(member) = member {
            return Ok(Callee::direct(member.code, name, Some((base, value))));
        }
        let selected = self.component_value(selection, value, &[name])?;
        self.value_callee(selection, selected)
    }

    pub(super) fn prepare_receiver(
        &mut self,
        base: NodeId,
        reachable: bool,
    ) -> Result<Receiver, Failure> {
        let (root, path) = self
            .outcome_receiver_path(base)?
            .unwrap_or((base, Vec::new()));
        Ok(Receiver {
            root: self.expression(root, None, reachable)?,
            path,
        })
    }

    pub(super) fn receiver_projection(
        &self,
        code: nocter_model::AccessCodeId,
    ) -> Result<Projection, Failure> {
        let program = self.program();
        let input = program
            .codes()
            .get(code)
            .and_then(|code| code.parameters().first())
            .and_then(|&id| program.parameters().get(id))
            .ok_or(Failure::InconsistentSyntax)?;
        Ok(Projection::input(input.operation().access_mode()))
    }

    pub(super) fn receiver_value(
        &mut self,
        receiver: Receiver,
        projection: Option<Projection>,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let mut value = receiver.root;
        for step in receiver.path {
            value = match step {
                Step::Outcome(node) => self.outcome_from_source(node, value, projection)?,
                Step::Recovery(node) => {
                    self.recover_from_source(node, value, projection, reachable)?
                }
                Step::Member { node, name } => self.component_value(node, value, &[name])?,
                Step::Index { node, suffix } => {
                    self.index_component(node, suffix, value, reachable)?
                }
            };
        }
        Ok(value)
    }

    fn outcome_receiver_path(&self, base: NodeId) -> Result<Option<(NodeId, Vec<Step>)>, Failure> {
        let mut root = self.unwrap(base)?;
        let mut path = Vec::new();
        let mut carrier = None;
        loop {
            let step = match self.kind(root)? {
                NodeKind::OutcomeExpression => Step::Outcome(root),
                NodeKind::RecoveryExpression => Step::Recovery(root),
                NodeKind::PostfixExpression => {
                    let children = self.children(root);
                    let [_, suffix] = children.as_slice() else {
                        return Err(Failure::InconsistentSyntax);
                    };
                    match self.kind(*suffix)? {
                        NodeKind::MemberSuffix => Step::Member {
                            node: root,
                            name: self.member_token(*suffix)?,
                        },
                        NodeKind::IndexSuffix => Step::Index {
                            node: root,
                            suffix: *suffix,
                        },
                        NodeKind::CallSuffix => break,
                        _ => return Err(Failure::InconsistentSyntax),
                    }
                }
                _ => break,
            };
            path.push(step);
            root = self.unwrap(self.first_child(root)?)?;
            if matches!(step, Step::Outcome(_) | Step::Recovery(_)) {
                carrier = Some((root, path.len()));
            }
        }
        let Some((carrier, length)) = carrier else {
            return Ok(None);
        };
        // The first carrier is an ordinary operand, including any qualified or static prefix.
        path.truncate(length);
        path.reverse();
        Ok(Some((carrier, path)))
    }

    pub(super) fn receiver_data(&self, receiver: &Receiver) -> Result<Option<TypeId>, Failure> {
        let mut data = self
            .ty(receiver.root)
            .access()
            .map(nocter_model::ValueAccess::data);
        for step in &receiver.path {
            let Some(current) = data else { return Ok(None) };
            data = match *step {
                Step::Outcome(_) | Step::Recovery(_) => match self.program().data_schema(current) {
                    Some(PayloadSchema::Optional(payload) | PayloadSchema::Fallible(payload)) => {
                        Some(payload)
                    }
                    _ => None,
                },
                Step::Member { name, .. } => Some(self.fields(current, &[name])?.1),
                Step::Index { .. } => match self.program().data_schema(current) {
                    Some(PayloadSchema::Array(array)) => Some(array.element()),
                    _ => None,
                },
            };
        }
        Ok(data)
    }
}
