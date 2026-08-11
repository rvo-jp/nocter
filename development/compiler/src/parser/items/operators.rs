use super::*;

impl Parser<'_> {
    pub(super) fn parse_equality_operator_decl(
        &mut self,
        visibility: Visibility,
    ) -> ParseResult<EqualityOperatorDecl> {
        let start = self.expect_keyword(Keyword::Operator, "`operator`")?;
        self.expect_punctuation("(", "`(`")?;
        let receiver =
            self.parse_self_receiver("expected readonly `&self` as the left equality operand")?;
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
        Ok(EqualityOperatorDecl::new(
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
        ))
    }
}
