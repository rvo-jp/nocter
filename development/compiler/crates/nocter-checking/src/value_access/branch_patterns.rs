//! Enum patterns select a variant in the type namespace before inspecting the source.
use super::{
    Failure, Rule,
    checker::Checker,
    ir::{Boundary, ExpressionKind, Projection, value_type},
};
use nocter_declaration_lowering::access_declarations::input_operation;
use nocter_model::{AccessExpressionId, BuiltinType, ValueAccessMode as Mode, ValueInputOperation};
use nocter_syntax::{NodeId, NodeKind, TokenKind, direct_token_iter};

impl Checker<'_, '_> {
    pub(super) fn if_condition(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let source_node = self.direct(node, NodeKind::Expression)?;
        let boolean = self.data(BuiltinType::Bool);
        let Ok(pattern) = self.direct(node, NodeKind::EnumPattern) else {
            let condition = self.expression(source_node, None, reachable)?;
            self.require_data(condition, boolean, source_node)?;
            return Ok(condition);
        };
        let path = direct_token_iter(self.syntax().tree(), pattern)
            .filter(|token| token.kind() == TokenKind::Identifier)
            .collect::<Vec<_>>();
        let (&member, owner) = path.split_last().ok_or(Failure::InconsistentSyntax)?;
        let data = self.annotation_data_path(owner, false)?;
        let variant = self
            .program()
            .behavior_owner(data)
            .map(|owner| self.enum_variant(owner, member))
            .transpose()?
            .flatten();
        let Some((data, tag)) = variant else {
            return self.reject_message(
                Rule::UnknownName,
                member,
                format!(
                    "{} has no variant `{}`",
                    self.data_label(data),
                    self.text(member)?
                ),
                Vec::new(),
            );
        };
        if let Ok(payload) = self.direct(pattern, NodeKind::EnumPatternPayload) {
            return self.reject_message(
                Rule::TypeMismatch,
                payload,
                "a payloadless enum pattern has no bindings".into(),
                Vec::new(),
            );
        }
        let operation =
            input_operation(self.syntax(), node).map_err(|_| Failure::InconsistentSyntax)?;
        let mut source = self.expression_with_type_hint(
            source_node,
            Some(data),
            Some(Projection::input(operation.access_mode())),
            reachable,
        )?;
        self.require_data(source, data, source_node)?;
        match operation {
            ValueInputOperation::Take => {
                source = self.boundary(source_node, source, Boundary::Value(data))?;
            }
            ValueInputOperation::Edit => {
                if let Some(place) = self.expression_referent(source) {
                    self.require_writable_referent(place, source_node, Rule::InvalidUpdate)?;
                }
            }
            ValueInputOperation::Read => {}
        }
        self.emit(
            node,
            value_type(boolean, Mode::Own).with_continuation(self.ty(source).may_complete()),
            ExpressionKind::EnumCondition {
                source,
                tag,
                operation,
            },
        )
    }
}
