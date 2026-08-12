use super::*;
use crate::ast::{CoercionEntry, MethodDecl, MethodReceiverMode, ParameterList};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_coercion_entry(
        &mut self,
        visibility: crate::ast::Visibility,
    ) -> ParseResult<CoercionEntry> {
        let keyword = self.expect_keyword(Keyword::Coerce, "`coerce`")?;
        let receiver = self.parse_coercion_receiver()?;
        let as_token = self.expect_keyword(Keyword::As, "`as`")?;
        let target = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let body = self
            .at_punctuation("{")
            .then(|| self.parse_block())
            .transpose()?;
        let end = body.as_ref().map_or_else(
            || {
                result_provenance
                    .as_ref()
                    .map_or(target.span().end, |clause| clause.span.end)
            },
            |body| body.span.end,
        );
        let span = self.span(keyword.span.start, end);
        let callable = MethodDecl {
            span,
            visibility,
            keyword_span: keyword.span,
            receiver,
            name: format!("__nocter$coerce${}", as_token.span.start),
            name_span: as_token.span,
            generics: crate::ast::GenericParamList::empty(),
            parameters: ParameterList {
                span: as_token.span,
                parameters: Vec::new(),
            },
            return_type: target,
            result_provenance,
            requirements: None,
            body,
        };
        Ok(CoercionEntry::new(
            span,
            keyword.span,
            as_token.span,
            callable,
        ))
    }

    fn parse_coercion_receiver(&mut self) -> ParseResult<crate::ast::MethodReceiver> {
        let receiver =
            self.parse_self_receiver("expected `&self` or `&+self` receiver in coercion entry")?;
        if receiver.mode == MethodReceiverMode::Owned {
            self.error_at(
                receiver.span,
                "coercion receiver must be borrowed; write `&self` or `&+self`",
            );
            return Err(());
        }
        Ok(receiver)
    }
}
