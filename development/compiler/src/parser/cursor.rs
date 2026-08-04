use super::support::{ParsedBinaryOperator, ParsedIdentifier, ParsedUnaryOperator};
use super::{ParseResult, Parser};
use crate::ast::{BinaryOperator, UnaryOperator};
use crate::diagnostics::Diagnostic;
use crate::lexer::{Keyword, Token, TokenKind};
use crate::source::ByteSpan;

impl Parser<'_> {
    pub(super) fn expect_identifier(&mut self, message: &str) -> ParseResult<ParsedIdentifier> {
        if self.current().kind == TokenKind::Identifier {
            let token = self.bump();
            return Ok(ParsedIdentifier {
                value: self.lexeme(&token),
                span: token.span,
            });
        }

        self.error_current(message);
        Err(())
    }

    pub(super) fn expect_name_identifier(
        &mut self,
        message: &str,
    ) -> ParseResult<ParsedIdentifier> {
        let identifier = self.expect_identifier(message)?;
        if let Some(message) = reserved_name_diagnostic(&identifier.value) {
            self.error_at(identifier.span, message);
            return Err(());
        }

        Ok(identifier)
    }

    pub(super) fn expect_integer_literal(&mut self, message: &str) -> ParseResult<Token> {
        if self.current().kind == TokenKind::IntegerLiteral {
            return Ok(self.bump());
        }

        self.error_current(message);
        Err(())
    }

    pub(super) fn expect_string_literal(&mut self, message: &str) -> ParseResult<Token> {
        if self.current().kind == TokenKind::StringLiteral {
            return Ok(self.bump());
        }

        self.error_current(message);
        Err(())
    }

    pub(super) fn expect_keyword(
        &mut self,
        keyword: Keyword,
        expected: &str,
    ) -> ParseResult<Token> {
        if self.at_keyword(keyword) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    pub(super) fn expect_punctuation(
        &mut self,
        punctuation: &str,
        expected: &str,
    ) -> ParseResult<Token> {
        if punctuation == ">"
            && matches!(self.current().kind, TokenKind::Punctuation(">>"))
            && self.pending_token.is_none()
        {
            let token = self.bump();
            let first = Token {
                kind: TokenKind::Punctuation(">"),
                span: self.span(token.span.start, token.span.start + 1),
            };
            self.pending_token = Some(Token {
                kind: TokenKind::Punctuation(">"),
                span: self.span(token.span.start + 1, token.span.end),
            });
            return Ok(first);
        }

        if self.at_punctuation(punctuation) {
            return Ok(self.bump());
        }

        self.error_current(format!("expected {expected}"));
        Err(())
    }

    pub(super) fn expect_statement_end(&mut self, message: &str) -> ParseResult<()> {
        if self.at_statement_end() {
            return Ok(());
        }

        self.error_current(message);
        Err(())
    }

    pub(super) fn match_keyword(&mut self, keyword: Keyword) -> Option<Token> {
        if self.at_keyword(keyword) {
            Some(self.bump())
        } else {
            None
        }
    }

    pub(super) fn match_identifier_text(&mut self, text: &str) -> Option<Token> {
        if self.at_identifier_text(text) {
            Some(self.bump())
        } else {
            None
        }
    }

    pub(super) fn match_punctuation(&mut self, punctuation: &str) -> Option<Token> {
        if self.at_punctuation(punctuation) {
            Some(self.bump())
        } else {
            None
        }
    }

    pub(super) fn match_logical_or_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("||") => BinaryOperator::LogicalOr,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_logical_and_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("&&") => BinaryOperator::LogicalAnd,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_equality_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("==") => BinaryOperator::Equal,
            TokenKind::Punctuation("!=") => BinaryOperator::NotEqual,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_ordering_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("<") => BinaryOperator::Less,
            TokenKind::Punctuation("<=") => BinaryOperator::LessEqual,
            TokenKind::Punctuation(">") => BinaryOperator::Greater,
            TokenKind::Punctuation(">=") => BinaryOperator::GreaterEqual,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_shift_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("<<") => BinaryOperator::ShiftLeft,
            TokenKind::Punctuation(">>") => BinaryOperator::ShiftRight,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_additive_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("+") => BinaryOperator::Add,
            TokenKind::Punctuation("-") => BinaryOperator::Subtract,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_multiplicative_operator(&mut self) -> Option<ParsedBinaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("*") => BinaryOperator::Multiply,
            TokenKind::Punctuation("/") => BinaryOperator::Divide,
            TokenKind::Punctuation("%") => BinaryOperator::Remainder,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedBinaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn match_unary_operator(&mut self) -> Option<ParsedUnaryOperator> {
        let value = match self.current().kind {
            TokenKind::Punctuation("!") => UnaryOperator::LogicalNot,
            TokenKind::Punctuation("-") => UnaryOperator::Negate,
            TokenKind::Keyword(Keyword::Move) => UnaryOperator::Move,
            _ => return None,
        };
        let token = self.bump();
        Some(ParsedUnaryOperator {
            value,
            span: token.span,
        })
    }

    pub(super) fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(actual) if actual == keyword)
    }

    pub(super) fn at_identifier_text(&self, text: &str) -> bool {
        self.current().kind == TokenKind::Identifier && self.lexeme(self.current()) == text
    }

    pub(super) fn at_punctuation(&self, punctuation: &str) -> bool {
        matches!(self.current().kind, TokenKind::Punctuation(actual) if actual == punctuation)
    }

    pub(super) fn at_ellipsis(&self) -> bool {
        self.at_punctuation(".")
            && self.next_is_punctuation(".")
            && matches!(
                self.token_kind_at_offset(2),
                Some(TokenKind::Punctuation("."))
            )
    }

    pub(super) fn ellipsis_span(&self) -> ByteSpan {
        let start = self.current().span.start;
        let end = self
            .token_at_offset(2)
            .map(|token| token.span.end)
            .unwrap_or_else(|| self.current().span.end);
        self.span(start, end)
    }

    pub(super) fn next_is_identifier(&self) -> bool {
        matches!(self.token_kind_at_offset(1), Some(TokenKind::Identifier))
    }

    pub(super) fn next_is_keyword(&self, keyword: Keyword) -> bool {
        matches!(
            self.token_kind_at_offset(1),
            Some(TokenKind::Keyword(actual)) if actual == keyword
        )
    }

    pub(super) fn current_touches_next_identifier(&self) -> bool {
        self.token_at_offset(1).is_some_and(|next| {
            next.kind == TokenKind::Identifier && self.current().span.end == next.span.start
        })
    }

    pub(super) fn current_touches_next_punctuation(&self, punctuation: &str) -> bool {
        self.token_at_offset(1).is_some_and(|next| {
            matches!(next.kind, TokenKind::Punctuation(actual) if actual == punctuation)
                && self.current().span.end == next.span.start
        })
    }

    pub(super) fn punctuation_at_offset(&self, offset: usize, punctuation: &str) -> bool {
        matches!(
            self.token_kind_at_offset(offset),
            Some(TokenKind::Punctuation(actual)) if actual == punctuation
        )
    }

    pub(super) fn next_is_punctuation(&self, punctuation: &str) -> bool {
        matches!(
            self.token_kind_at_offset(1),
            Some(TokenKind::Punctuation(actual)) if actual == punctuation
        )
    }

    pub(super) fn looks_like_struct_literal_body(&self) -> bool {
        self.looks_like_struct_literal_body_at(self.index)
    }

    pub(super) fn looks_like_generic_struct_literal_body(&self) -> bool {
        if !self.at_punctuation("<") {
            return false;
        }

        let mut depth = 0usize;
        let mut index = self.index;
        loop {
            match self.tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Punctuation("<")) => {
                    depth += 1;
                    index += 1;
                }
                Some(TokenKind::Punctuation(">")) => {
                    depth = depth.saturating_sub(1);
                    index += 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(TokenKind::Punctuation(">>")) => {
                    if depth < 2 {
                        return false;
                    }
                    depth -= 2;
                    index += 1;
                    if depth == 0 {
                        break;
                    }
                }
                Some(TokenKind::Eof) | None => return false,
                _ => index += 1,
            }
        }

        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }

        self.looks_like_struct_literal_body_at(index)
    }

    pub(super) fn looks_like_typed_sequence_literal_start(&self) -> bool {
        self.typed_literal_shape_token().is_some_and(|token| {
            matches!(token.kind, TokenKind::Punctuation("["))
                && self
                    .typed_literal_target_end()
                    .is_some_and(|end| end < token.span.start)
        })
    }

    pub(super) fn looks_like_typed_string_literal_start(&self) -> bool {
        self.typed_literal_shape_token().is_some_and(|token| {
            token.kind == TokenKind::StringLiteral
                && self
                    .typed_literal_target_end()
                    .is_some_and(|end| end < token.span.start)
        })
    }

    pub(super) fn looks_like_closure_expression(&self) -> bool {
        if !self.at_punctuation("(") || self.pending_token.is_some() {
            return false;
        }
        let mut first = self.index + 1;
        while matches!(
            self.tokens.get(first).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            first += 1;
        }
        match self.tokens.get(first).map(|token| token.kind) {
            Some(TokenKind::Punctuation(")")) => {}
            Some(TokenKind::Identifier) => {
                let mut after_name = first + 1;
                while matches!(
                    self.tokens.get(after_name).map(|token| token.kind),
                    Some(TokenKind::Newline)
                ) {
                    after_name += 1;
                }
                if !matches!(
                    self.tokens.get(after_name).map(|token| token.kind),
                    Some(
                        TokenKind::Punctuation(",")
                            | TokenKind::Punctuation(":")
                            | TokenKind::Punctuation(")")
                    )
                ) {
                    return false;
                }
            }
            Some(
                TokenKind::Punctuation("&")
                | TokenKind::Punctuation("&+")
                | TokenKind::Keyword(crate::lexer::Keyword::Move),
            ) => {}
            _ => return false,
        }
        let mut depth = 0usize;
        let mut index = self.index;
        loop {
            match self.tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Punctuation("(")) => depth += 1,
                Some(TokenKind::Punctuation(")")) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        index += 1;
                        break;
                    }
                }
                Some(TokenKind::Eof) | None => return false,
                _ => {}
            }
            index += 1;
        }
        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }
        matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Punctuation("{") | TokenKind::Punctuation(":"))
        )
    }

    pub(super) fn closure_header_has_capture_separator(&self) -> bool {
        if self.pending_token.is_some() {
            return false;
        }
        let mut index = self.index;
        loop {
            match self.tokens.get(index).map(|token| token.kind) {
                Some(TokenKind::Punctuation(";")) => return true,
                Some(TokenKind::Punctuation(")")) | Some(TokenKind::Eof) | None => return false,
                _ => index += 1,
            }
        }
    }

    fn typed_literal_shape_token(&self) -> Option<&Token> {
        let index = self.typed_literal_shape_index()?;
        self.tokens.get(index)
    }

    fn typed_literal_target_end(&self) -> Option<usize> {
        let shape_index = self.typed_literal_shape_index()?;
        shape_index
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|token| token.span.end)
    }

    fn typed_literal_shape_index(&self) -> Option<usize> {
        if self.current().kind != TokenKind::Identifier {
            return None;
        }
        let mut index = self.index + 1;
        if !matches!(self.tokens.get(index)?.kind, TokenKind::Punctuation("<")) {
            return Some(index);
        }

        let mut depth = 0usize;
        while let Some(token) = self.tokens.get(index) {
            match token.kind {
                TokenKind::Punctuation("<") => depth += 1,
                TokenKind::Punctuation(">") => {
                    depth = depth.checked_sub(1)?;
                    if depth == 0 {
                        return Some(index + 1);
                    }
                }
                TokenKind::Punctuation(">>") => {
                    if depth <= 2 {
                        return Some(index + 1);
                    }
                    depth -= 2;
                }
                TokenKind::Eof => return None,
                _ => {}
            }
            index += 1;
        }
        None
    }

    fn looks_like_struct_literal_body_at(&self, start: usize) -> bool {
        if !matches!(
            self.tokens.get(start).map(|token| token.kind),
            Some(TokenKind::Punctuation("{"))
        ) {
            return false;
        }

        let mut index = start + 1;
        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }

        if self.tokens_start_ellipsis_at(index) {
            return true;
        }

        if !matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Identifier)
        ) {
            return false;
        }

        index += 1;
        while matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Newline)
        ) {
            index += 1;
        }

        matches!(
            self.tokens.get(index).map(|token| token.kind),
            Some(TokenKind::Punctuation(":"))
        )
    }

    pub(super) fn at_statement_end(&self) -> bool {
        self.current().kind == TokenKind::Newline || self.at_punctuation("}") || self.at_eof()
    }

    pub(super) fn at_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }

    pub(super) fn skip_newlines(&mut self) {
        while self.current().kind == TokenKind::Newline {
            self.bump();
        }
    }

    pub(super) fn current(&self) -> &Token {
        if let Some(token) = &self.pending_token {
            return token;
        }

        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("parser requires an EOF token"))
    }

    pub(super) fn bump(&mut self) -> Token {
        if let Some(token) = self.pending_token.take() {
            return token;
        }

        let token = self.current().clone();
        if !self.at_eof() {
            self.index += 1;
        }
        token
    }

    pub(super) fn span(&self, start: usize, end: usize) -> ByteSpan {
        ByteSpan::new(self.source, start, end)
    }

    pub(super) fn lexeme(&self, token: &Token) -> String {
        self.sources
            .get(token.span.source)
            .and_then(|file| file.text().get(token.span.start..token.span.end))
            .unwrap_or("")
            .to_string()
    }

    fn token_at_offset(&self, offset: usize) -> Option<&Token> {
        if self.pending_token.is_some() {
            if offset == 0 {
                return self.pending_token.as_ref();
            }
            return self.tokens.get(self.index + offset - 1);
        }

        self.tokens.get(self.index + offset)
    }

    fn token_kind_at_index(&self, index: usize) -> Option<TokenKind> {
        self.tokens.get(index).map(|token| token.kind)
    }

    fn token_kind_at_offset(&self, offset: usize) -> Option<TokenKind> {
        self.token_at_offset(offset).map(|token| token.kind)
    }

    fn tokens_start_ellipsis_at(&self, index: usize) -> bool {
        matches!(
            self.token_kind_at_index(index),
            Some(TokenKind::Punctuation("."))
        ) && matches!(
            self.token_kind_at_index(index + 1),
            Some(TokenKind::Punctuation("."))
        ) && matches!(
            self.token_kind_at_index(index + 2),
            Some(TokenKind::Punctuation("."))
        )
    }

    pub(super) fn error_current(&mut self, message: impl Into<String>) {
        let token = self.current();
        self.error_at(token.span, message);
    }

    pub(super) fn error_at(&mut self, span: ByteSpan, message: impl Into<String>) {
        let primary_span = self
            .sources
            .span_to_json(ByteSpan::new(span.source, span.start, span.end))
            .ok()
            .map(Box::new);
        let mut diagnostic = Diagnostic::error("E0200", message);
        diagnostic.primary_span = primary_span;
        self.diagnostics.push(diagnostic);
    }
}

fn reserved_name_diagnostic(name: &str) -> Option<&'static str> {
    match name {
        "_" => Some(
            "`_` is reserved for discard or wildcard patterns and cannot be used as a name in v0",
        ),
        "Self" => {
            Some("`Self` is reserved as contextual type syntax and cannot be used as a name in v0")
        }
        _ => None,
    }
}
