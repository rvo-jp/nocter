use nocter_declarations::ParameterRole;
use nocter_model::{ParameterId, TypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_identifier, direct_nodes, is_transparent_expression};
use crate::{CheckedOperation, NameTarget};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ArgumentPackUse {
    Iterated,
    Forwarded,
}

impl BodyChecker<'_, '_> {
    pub(super) fn register_argument_pack_iteration(
        &mut self,
        parameter: ParameterId,
        syntax: NodeId,
    ) -> Result<(), BodyCheckError> {
        if self.argument_pack_uses.get(&parameter) == Some(&ArgumentPackUse::Forwarded) {
            return Err(self.rule(BodyRule::InvalidArgumentPackUse, syntax)?);
        }
        self.argument_pack_uses
            .insert(parameter, ArgumentPackUse::Iterated);
        Ok(())
    }

    pub(super) fn register_argument_pack_forwarding(
        &mut self,
        parameter: ParameterId,
        syntax: NodeId,
    ) -> Result<(), BodyCheckError> {
        if self.argument_pack_uses.contains_key(&parameter) {
            return Err(self.rule(BodyRule::InvalidArgumentPackUse, syntax)?);
        }
        self.argument_pack_uses
            .insert(parameter, ArgumentPackUse::Forwarded);
        Ok(())
    }

    /// Recognizes and consumes a direct reference to the current callable's argument pack.
    /// Ordinary parameters are left untouched for the normal expression path.
    pub(super) fn argument_pack_parameter(
        &mut self,
        mut syntax: NodeId,
    ) -> Result<Option<(ParameterId, TypeId)>, BodyCheckError> {
        while self.kind(syntax).is_ok_and(is_transparent_expression) {
            let children = direct_nodes(self.tree(), syntax);
            let [child] = children.as_slice() else {
                return Ok(None);
            };
            syntax = *child;
        }
        if self.kind(syntax)? != NodeKind::ReferenceExpression {
            return Ok(None);
        }
        let Some(token) = direct_identifier(self.tree(), syntax) else {
            return Ok(None);
        };
        let origin = SyntaxOrigin::Token(token);
        let Some(NameTarget::Parameter(parameter)) = self.uses.get(&origin).copied() else {
            return Ok(None);
        };
        let declaration = self
            .graph
            .declarations()
            .parameters()
            .get(parameter)
            .copied()
            .ok_or(BodyCheckInternalError::MissingParameterType(
                NameTarget::Parameter(parameter),
            ))?;
        if !matches!(declaration.role(), ParameterRole::ArgumentPack { .. }) {
            return Ok(None);
        }
        self.consumed_uses.insert(origin);
        Ok(Some((parameter, declaration.ty())))
    }

    pub(super) fn check_argument_pack_method(
        &mut self,
        node: NodeId,
        parameter: ParameterId,
        member: NodeId,
        suffix: NodeId,
        expected: Option<TypeId>,
    ) -> Result<nocter_model::BodyNodeId, BodyCheckError> {
        let member = direct_identifier(self.tree(), member)
            .ok_or(BodyCheckInternalError::InvalidSyntax(member))?;
        if self.token_text(member)? != "len" || !direct_nodes(self.tree(), suffix).is_empty() {
            return Err(self.rule(BodyRule::InvalidArgumentPackUse, node)?);
        }
        let ty = self.types.builtin(nocter_model::BuiltinType::Usize);
        let value = self.add_node(node, ty, CheckedOperation::ArgumentPackLength(parameter))?;
        expected.map_or(Ok(value), |expected| {
            self.apply_expected(node, value, expected)
        })
    }
}
