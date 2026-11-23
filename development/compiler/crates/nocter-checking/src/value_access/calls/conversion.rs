//! Explicit conversion selects exact data before applying receiver access or value supply.
use super::{AccessExpressionId, Callee, Checker, Failure, NodeId, Rule, TokenKind, TypeId};
use crate::value_access::{annotations::Annotation, ir::Boundary};
use nocter_declaration_lowering::access_declarations::ConversionSelection;
use nocter_syntax::{Keyword, direct_token};

impl Checker<'_, '_> {
    pub(in crate::value_access) fn conversion(
        &mut self,
        node: NodeId,
        reachable: bool,
    ) -> Result<AccessExpressionId, Failure> {
        let children = self.children(node);
        let &[source, target] = children.as_slice() else {
            return Err(Failure::InconsistentSyntax);
        };
        let annotation = self.annotation(target)?;
        let receiver = self.prepare_receiver(source, reachable)?;
        if let Annotation::Data(data) = annotation
            && let Some(actual) = self.receiver_data(&receiver)?
            && actual != data
        {
            let token = direct_token(self.syntax().tree(), node, TokenKind::Keyword(Keyword::As))
                .ok_or(Failure::InconsistentSyntax)?;
            let member = match self
                .program()
                .behavior_owner(actual)
                .map_or(ConversionSelection::Missing, |owner| {
                    self.context.conversion(owner, data)
                }) {
                ConversionSelection::Selected(member) => member,
                ConversionSelection::Require(candidates) => {
                    return Err(Failure::RequiredHeaderCandidates {
                        candidates,
                        origin: nocter_syntax::SyntaxOrigin::Token(token),
                    });
                }
                ConversionSelection::Missing => {
                    return self.missing_conversion(target, actual, data);
                }
                ConversionSelection::Ambiguous => {
                    return self.reject_message(
                        Rule::TypeMismatch,
                        target,
                        "more than one declaration converts to the same target data".into(),
                        Vec::new(),
                    );
                }
            };
            self.require_type_member(member, token, true)?;
            let projection = self.receiver_projection(member.code)?;
            let value = self.receiver_value(receiver, Some(projection), reachable)?;
            let selected = Callee::direct(member.code, token, Some((source, value)));
            return self.selected_call(node, &[], &selected, None, reachable);
        }

        let value = self.receiver_value(receiver, None, reachable)?;
        let data = match annotation {
            Annotation::Data(data) => data,
            Annotation::Function(pattern) => {
                if self.ty(value).access().is_none() {
                    return Ok(value);
                }
                self.function_pattern_data(target, value, &pattern, false)?
            }
        };
        self.require_data(value, data, target)?;
        self.boundary(node, value, Boundary::Value(data))
    }

    fn missing_conversion(
        &self,
        target: NodeId,
        actual: TypeId,
        expected: TypeId,
    ) -> Result<AccessExpressionId, Failure> {
        self.reject_message(
            Rule::TypeMismatch,
            target,
            format!(
                "no conversion from {} to {}",
                self.data_label(actual),
                self.data_label(expected),
            ),
            Vec::new(),
        )
    }
}
