use nocter_declarations::ParameterRole;
use nocter_model::{ParameterId, TypeId};
use nocter_source_index::SyntaxOrigin;
use nocter_syntax::{NodeId, NodeKind};

use super::BodyChecker;
use crate::body_check::diagnostic::BodyRule;
use crate::body_check::error::{BodyCheckError, BodyCheckInternalError};
use crate::syntax::{direct_identifier, direct_nodes, is_transparent_expression};
use crate::{CheckedOperation, NameTarget};

impl BodyChecker<'_, '_> {
    /// Recognizes and consumes a direct reference to the current literal body's variadic pack.
    /// Ordinary parameters are left untouched for the normal expression path.
    pub(super) fn literal_pack_parameter(
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
        if declaration.role()
            != (ParameterRole::Ordinary {
                position: 0,
                variadic: true,
            })
        {
            return Ok(None);
        }
        self.consumed_uses.insert(origin);
        Ok(Some((parameter, declaration.ty())))
    }

    pub(super) fn check_literal_pack_method(
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
            return Err(self.rule(BodyRule::InvalidLiteralPackUse, node)?);
        }
        let ty = self.types.builtin(nocter_model::BuiltinType::Usize);
        let value = self.add_node(node, ty, CheckedOperation::LiteralPackLength(parameter))?;
        expected.map_or(Ok(value), |expected| {
            self.apply_expected(node, value, expected)
        })
    }
}
