use super::*;

impl Parser<'_> {
    pub(super) fn parse_method_decl(
        &mut self,
        visibility: Visibility,
        _require_body: bool,
    ) -> ParseResult<MethodDecl> {
        let start = self.expect_keyword(Keyword::Method, "`method`")?;
        let receiver = self.parse_method_receiver()?;
        self.expect_punctuation(".", "`.`")?;
        let name = self.expect_name_identifier("expected method name after `.`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let requirements = self.parse_where_clause()?;
        let body = self
            .at_punctuation("{")
            .then(|| self.parse_block())
            .transpose()?;
        let end = body.as_ref().map_or_else(
            || {
                requirements.as_ref().map_or_else(
                    || {
                        result_provenance
                            .as_ref()
                            .map_or(return_type.span().end, |clause| clause.span.end)
                    },
                    |clause| clause.span.end,
                )
            },
            |body| body.span.end,
        );
        Ok(MethodDecl {
            name: name.value,
            name_span: name.span,
            callable: crate::ast::CallableDecl {
                span: self.span(start.span.start, end),
                visibility,
                keyword_span: start.span,
                receiver,
                generics,
                parameters,
                return_type,
                result_provenance,
                requirements,
                body,
            },
        })
    }

    fn parse_method_receiver(&mut self) -> ParseResult<MethodReceiver> {
        self.parse_self_receiver("expected `self`, `&self`, or `&+self` receiver after `method`")
    }

    pub(super) fn parse_self_receiver(
        &mut self,
        message: &'static str,
    ) -> ParseResult<MethodReceiver> {
        let borrow = self
            .match_punctuation("&+")
            .map(|token| (token, true))
            .or_else(|| self.match_punctuation("&").map(|token| (token, false)));
        let self_span = self.expect_self_identifier(message)?;
        let (span, mode) = if let Some((borrow, is_readwrite)) = borrow {
            (
                self.span(borrow.span.start, self_span.end),
                if is_readwrite {
                    MethodReceiverMode::ReadwriteBorrow
                } else {
                    MethodReceiverMode::ReadonlyBorrow
                },
            )
        } else {
            (self_span, MethodReceiverMode::Owned)
        };
        Ok(MethodReceiver {
            span,
            name: "self".to_string(),
            name_span: self_span,
            mode,
        })
    }

    pub(super) fn expect_self_identifier(
        &mut self,
        message: impl Into<String>,
    ) -> ParseResult<ByteSpan> {
        let message = message.into();
        let identifier = self.expect_identifier(&message)?;
        if identifier.value != "self" {
            self.error_at(identifier.span, "receiver name must be `self`");
            return Err(());
        }
        Ok(identifier.span)
    }
}
