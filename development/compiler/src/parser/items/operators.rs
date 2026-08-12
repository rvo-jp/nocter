use super::*;

impl Parser<'_> {
    pub(super) fn parse_operator_decl(
        &mut self,
        visibility: Visibility,
    ) -> ParseResult<OperatorDecl> {
        let start = self.expect_keyword(Keyword::Operator, "`operator`")?;
        self.expect_punctuation("(", "`(`")?;
        if self.at_ellipsis() {
            let operator_span = self.ellipsis_span();
            self.bump();
            self.bump();
            self.bump();
            return self.parse_expansion_operator_decl(start, operator_span, visibility);
        }
        let receiver =
            self.parse_self_receiver("expected `&self` or `&+self` operator receiver")?;
        if self.at_punctuation("[") {
            return self.parse_index_operator_decl(start, visibility, receiver);
        }
        self.parse_comparison_operator_decl(start, visibility, receiver)
    }

    fn parse_expansion_operator_decl(
        &mut self,
        start: Token,
        operator_span: crate::source::ByteSpan,
        visibility: Visibility,
    ) -> ParseResult<OperatorDecl> {
        let receiver = self.parse_self_receiver(
            "expected `self`, `&self`, or `&+self` after expansion operator",
        )?;
        self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let body = self.parse_block()?;
        let span = self.span(start.span.start, body.span.end);
        Ok(OperatorDecl::Expansion(ExpansionOperatorDecl::new(
            span,
            operator_span,
            crate::ast::CallableDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                generics: crate::ast::GenericParamList::empty(),
                parameters: ParameterList {
                    span: operator_span,
                    parameters: Vec::new(),
                },
                return_type,
                result_provenance,
                requirements: None,
                body: Some(body),
            },
        )))
    }

    fn parse_comparison_operator_decl(
        &mut self,
        start: Token,
        visibility: Visibility,
        receiver: MethodReceiver,
    ) -> ParseResult<OperatorDecl> {
        if receiver.mode != MethodReceiverMode::ReadonlyBorrow {
            self.error_at(
                receiver.span,
                "comparison left operand must be readonly `&self`",
            );
            return Err(());
        }
        let (operator, kind) = if let Some(operator) = self.match_punctuation("==") {
            (operator, ComparisonOperatorKind::Equality)
        } else if let Some(operator) = self.match_punctuation("<") {
            (operator, ComparisonOperatorKind::StrictOrder)
        } else {
            self.error_current("expected `==` or `<` after comparison receiver");
            return Err(());
        };
        let name = self.expect_name_identifier(&format!(
            "expected right operand name after `{}`",
            kind.source_token()
        ))?;
        self.expect_punctuation(":", "`:`")?;
        let ty = self.parse_type()?;
        let other = Parameter {
            span: self.span(name.span.start, ty.span().end),
            name: name.value,
            name_span: name.span,
            ty,
        };
        self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let span = self.span(start.span.start, body.span.end);
        Ok(OperatorDecl::Comparison(ComparisonOperatorDecl::new(
            span,
            operator.span,
            kind,
            crate::ast::CallableDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                generics: crate::ast::GenericParamList::empty(),
                parameters: ParameterList {
                    span: other.span,
                    parameters: vec![other],
                },
                return_type,
                result_provenance: None,
                requirements: None,
                body: Some(body),
            },
        )))
    }

    fn parse_index_operator_decl(
        &mut self,
        start: Token,
        visibility: Visibility,
        receiver: MethodReceiver,
    ) -> ParseResult<OperatorDecl> {
        let open_bracket = self.expect_punctuation("[", "`[`")?;
        let name = self.expect_name_identifier("expected index parameter name after `[")?;
        self.expect_punctuation(":", "`:`")?;
        let ty = self.parse_type()?;
        let parameter = Parameter {
            span: self.span(name.span.start, ty.span().end),
            name: name.value,
            name_span: name.span,
            ty,
        };
        let close_bracket = self.expect_punctuation("]", "`]`")?;
        self.expect_punctuation(")", "`)`")?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let body = self.parse_block()?;
        let span = self.span(start.span.start, body.span.end);
        match receiver.mode {
            MethodReceiverMode::ReadonlyBorrow | MethodReceiverMode::ReadwriteBorrow => {}
            MethodReceiverMode::Owned => {
                self.error_at(receiver.span, "index receiver must be `&self` or `&+self`");
                return Err(());
            }
        };
        Ok(OperatorDecl::Index(IndexOperatorDecl::new(
            span,
            start.span,
            open_bracket.span,
            close_bracket.span,
            crate::ast::CallableDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                generics: crate::ast::GenericParamList::empty(),
                parameters: ParameterList {
                    span: parameter.span,
                    parameters: vec![parameter],
                },
                return_type,
                result_provenance,
                requirements: None,
                body: Some(body),
            },
        )))
    }
}
