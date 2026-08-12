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
        self.parse_equality_operator_decl(start, visibility, receiver)
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
        let name = match receiver.mode {
            MethodReceiverMode::ReadonlyBorrow => {
                crate::ast::READONLY_EXPANSION_OPERATOR_METHOD_NAME
            }
            MethodReceiverMode::ReadwriteBorrow => {
                crate::ast::READWRITE_EXPANSION_OPERATOR_METHOD_NAME
            }
            MethodReceiverMode::Owned => crate::ast::OWNED_EXPANSION_OPERATOR_METHOD_NAME,
        };
        Ok(OperatorDecl::Expansion(ExpansionOperatorDecl::new(
            span,
            operator_span,
            MethodDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                name: name.to_string(),
                name_span: operator_span,
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

    fn parse_equality_operator_decl(
        &mut self,
        start: Token,
        visibility: Visibility,
        receiver: MethodReceiver,
    ) -> ParseResult<OperatorDecl> {
        if receiver.mode != MethodReceiverMode::ReadonlyBorrow {
            self.error_at(
                receiver.span,
                "equality left operand must be readonly `&self`",
            );
            return Err(());
        }
        let operator = self.expect_punctuation("==", "`==`")?;
        let name = self.expect_name_identifier("expected right operand name after `==`")?;
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
        Ok(OperatorDecl::Equality(EqualityOperatorDecl::new(
            span,
            operator.span,
            MethodDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                name: crate::ast::EQUALITY_OPERATOR_METHOD_NAME.to_string(),
                name_span: operator.span,
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
        let name = match receiver.mode {
            MethodReceiverMode::ReadonlyBorrow => crate::ast::READONLY_INDEX_OPERATOR_METHOD_NAME,
            MethodReceiverMode::ReadwriteBorrow => crate::ast::READWRITE_INDEX_OPERATOR_METHOD_NAME,
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
            MethodDecl {
                span,
                visibility,
                keyword_span: start.span,
                receiver,
                name: name.to_string(),
                name_span: open_bracket.span,
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
