use super::*;

impl Parser<'_> {
    pub(in crate::parser) fn parse_parameter_list(&mut self) -> ParseResult<ParameterList> {
        let start = self.expect_punctuation("(", "`(`")?;
        let mut parameters = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation(")") {
            if self.at_eof() {
                self.error_current("expected `)` to close parameter list");
                return Err(());
            }

            if let Some(token) = self.match_keyword(Keyword::Var) {
                self.error_at(token.span, "parameters cannot use `var`");
                return Err(());
            }
            if self.at_ellipsis() {
                self.error_at(
                    self.ellipsis_span(),
                    "ordinary parameters cannot use variadic syntax",
                );
                return Err(());
            }
            let name = self.expect_name_identifier("expected parameter name")?;
            self.expect_punctuation(":", "`:`")?;
            let ty = self.parse_type()?;
            if self.at_punctuation("=") {
                self.error_current("parameters cannot declare default values");
                return Err(());
            }
            if self.at_ellipsis() {
                self.error_at(
                    self.ellipsis_span(),
                    "ordinary parameters cannot use variadic syntax",
                );
                return Err(());
            }
            let end = ty.span().end;
            parameters.push(Parameter {
                span: self.span(name.span.start, end),
                name: name.value,
                name_span: name.span,
                ty,
            });

            self.skip_newlines();
            let Some(comma) = self.match_punctuation(",") else {
                break;
            };
            if self.at_punctuation(")") {
                self.error_at(
                    comma.span,
                    "single-line parameter lists cannot have a trailing comma",
                );
                return Err(());
            }
            self.skip_newlines();
        }

        let end = self.expect_punctuation(")", "`)`")?;
        Ok(ParameterList {
            span: self.span(start.span.start, end.span.end),
            parameters,
        })
    }
}
