use super::*;
use crate::ast::{DirectiveField, DirectiveValue, PackageDirective, PackageFile, PackageManifest};
use crate::lexer::{Keyword, TokenKind};
use crate::literals::{decode_integer_literal_value, decode_string_literal_bytes};

impl Parser<'_> {
    pub(in crate::parser) fn parse_package_file(&mut self) -> ParseResult<PackageFile> {
        let start = self.current().span.start;
        let mut directives = Vec::new();
        while self.at_package_directive_start() {
            directives.push(self.parse_package_directive()?);
            if !self.at_statement_end() {
                self.error_current("expected a newline after package directive");
                return Err(());
            }
            self.skip_newlines();
        }
        let manifest_end = directives
            .last()
            .map_or(start, |directive| directive.span.end);
        let manifest = PackageManifest {
            span: self.span(start, manifest_end),
            directives,
        };
        if !self.at_eof() {
            self.error_current(
                "package files may contain package directives only; put Nocter code in `index.nct`",
            );
            return Err(());
        }
        Ok(PackageFile {
            span: self.span(start, manifest_end),
            manifest,
        })
    }

    fn at_package_directive_start(&self) -> bool {
        self.at_punctuation("#")
            && (self
                .identifier_text_at_offset(1)
                .is_some_and(|name| name != "target")
                || self.token_at_offset_is_keyword(1, Keyword::Test))
    }

    fn parse_package_directive(&mut self) -> ParseResult<PackageDirective> {
        let start = self.expect_punctuation("#", "`#`")?;
        let name = if self.at_keyword(Keyword::Test) {
            let token = self.bump();
            ParsedIdentifier {
                span: token.span,
                value: "test".to_string(),
            }
        } else {
            self.expect_identifier("expected package directive name after `#`")?
        };
        self.expect_punctuation(":", "`:` after package directive name")?;
        let value = self.parse_directive_value()?;
        Ok(PackageDirective {
            span: self.span(start.span.start, value.span().end),
            name_span: name.span,
            name: name.value,
            value,
        })
    }

    fn parse_directive_value(&mut self) -> ParseResult<DirectiveValue> {
        match self.current().kind {
            TokenKind::StringLiteral => self.parse_directive_string(),
            TokenKind::IntegerLiteral => self.parse_directive_integer(),
            TokenKind::Keyword(Keyword::True) | TokenKind::Keyword(Keyword::False) => {
                let token = self.bump();
                Ok(DirectiveValue::Boolean {
                    span: token.span,
                    value: token.kind == TokenKind::Keyword(Keyword::True),
                })
            }
            TokenKind::Punctuation("[") => self.parse_directive_list(),
            TokenKind::Punctuation("{") => self.parse_directive_record(),
            _ => {
                self.error_current(
                    "expected a directive string, integer, boolean, list, or record",
                );
                Err(())
            }
        }
    }

    fn parse_directive_string(&mut self) -> ParseResult<DirectiveValue> {
        let token = self.expect_string_literal("expected directive string")?;
        let source = self.lexeme(&token);
        let bytes = decode_string_literal_bytes(&source).map_err(|message| {
            self.error_at(token.span, format!("invalid directive string: {message}"));
        })?;
        let value = String::from_utf8(bytes).map_err(|_| {
            self.error_at(token.span, "directive strings must be UTF-8");
        })?;
        Ok(DirectiveValue::String {
            span: token.span,
            content_span: self.span(token.span.start + 1, token.span.end.saturating_sub(1)),
            value,
        })
    }

    fn parse_directive_integer(&mut self) -> ParseResult<DirectiveValue> {
        let token = self.expect_integer_literal("expected directive integer")?;
        let source = self.lexeme(&token);
        let Some(value) = decode_integer_literal_value(&source) else {
            self.error_at(token.span, "directive integer is out of range");
            return Err(());
        };
        Ok(DirectiveValue::Integer {
            span: token.span,
            value,
        })
    }

    fn parse_directive_list(&mut self) -> ParseResult<DirectiveValue> {
        let open = self.expect_punctuation("[", "`[`")?;
        let mut values = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation("]") {
            values.push(self.parse_directive_value()?);
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }
        let close = self.expect_punctuation("]", "`]`")?;
        Ok(DirectiveValue::List {
            span: self.span(open.span.start, close.span.end),
            values,
        })
    }

    fn parse_directive_record(&mut self) -> ParseResult<DirectiveValue> {
        let open = self.expect_punctuation("{", "`{`")?;
        let mut fields = Vec::new();
        self.skip_newlines();
        while !self.at_punctuation("}") {
            let name = self.expect_identifier("expected directive field name")?;
            self.expect_punctuation(":", "`:` after directive field name")?;
            let value = self.parse_directive_value()?;
            fields.push(DirectiveField {
                span: self.span(name.span.start, value.span().end),
                name_span: name.span,
                name: name.value,
                value,
            });
            self.skip_newlines();
            if self.match_punctuation(",").is_none() {
                break;
            }
            self.skip_newlines();
        }
        let close = self.expect_punctuation("}", "`}`")?;
        Ok(DirectiveValue::Record {
            span: self.span(open.span.start, close.span.end),
            fields,
        })
    }
}
