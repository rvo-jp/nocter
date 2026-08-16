mod block;
mod declaration;
mod expression;
mod newline;
mod place;
mod requirements;
mod root;
#[cfg(test)]
mod snapshots;
mod statement;
#[cfg(test)]
mod tests;
mod types;

use nocter_source::{SourceFile, Span, TextRange};

use crate::tree::{Event, build_tree, missing};
use crate::{
    ExpectedSyntax, Keyword, NodeKind, ParseDiagnostic, ParseDiagnosticKind, Punctuation,
    SyntaxToken, SyntaxTree, Token, TokenId, TokenKind, lex,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParseGoal {
    PackageFile,
    ModuleSource,
}

#[must_use]
pub fn parse(source: &SourceFile, goal: ParseGoal) -> SyntaxTree {
    let lexed = lex(source);
    let mut parser = Parser::new(source, lexed.tokens());
    match goal {
        ParseGoal::PackageFile => root::package_file(&mut parser),
        ParseGoal::ModuleSource => root::module_source(&mut parser),
    }
    let (events, diagnostics) = parser.finish();
    let built = build_tree(source.id(), &events);
    SyntaxTree::new(lexed, built, diagnostics)
}

struct Parser<'source> {
    source: &'source SourceFile,
    tokens: &'source [Token],
    cursor: usize,
    events: Vec<Event>,
    diagnostics: Vec<ParseDiagnostic>,
    nesting: u16,
    split: Option<SyntaxToken>,
}

impl<'source> Parser<'source> {
    const MAX_NESTING: u16 = 256;

    fn new(source: &'source SourceFile, tokens: &'source [Token]) -> Self {
        Self {
            source,
            tokens,
            cursor: 0,
            events: Vec::new(),
            diagnostics: Vec::new(),
            nesting: 0,
            split: None,
        }
    }

    fn finish(self) -> (Vec<Event>, Vec<ParseDiagnostic>) {
        assert!(
            self.split.is_none(),
            "parser left a subdivided token unconsumed"
        );
        (self.events, self.diagnostics)
    }

    fn start(&mut self) -> Marker {
        let event_index = self.events.len();
        self.events.push(Event::Start {
            kind: NodeKind::Error,
            offset: self.current_span().range().start(),
            forward_parent: None,
        });
        Marker { event_index }
    }

    fn complete(&mut self, marker: Marker, kind: NodeKind) -> CompletedMarker {
        let Event::Start {
            kind: event_kind, ..
        } = &mut self.events[marker.event_index]
        else {
            unreachable!("marker must point to a start event");
        };
        *event_kind = kind;
        self.events.push(Event::Finish);
        CompletedMarker {
            event_index: marker.event_index,
        }
    }

    fn precede(&mut self, completed: CompletedMarker) -> Marker {
        let event_index = self.events.len();
        let Event::Start {
            offset,
            forward_parent,
            ..
        } = &mut self.events[completed.event_index]
        else {
            unreachable!("completed marker must point to a start event");
        };
        assert!(
            forward_parent.is_none(),
            "a completed node can acquire only one direct forward parent"
        );
        *forward_parent = Some(event_index - completed.event_index);
        let offset = *offset;
        self.events.push(Event::Start {
            kind: NodeKind::Error,
            offset,
            forward_parent: None,
        });
        Marker { event_index }
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.current_kind() == kind
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        self.at(TokenKind::Keyword(keyword))
    }

    fn at_punctuation(&self, punctuation: Punctuation) -> bool {
        self.at(TokenKind::Punctuation(punctuation))
    }

    fn at_identifier_text(&self, text: &str) -> bool {
        self.at(TokenKind::Identifier) && self.current_text() == text
    }

    fn nth_kind(&self, distance: usize) -> TokenKind {
        assert!(
            self.split.is_none(),
            "raw lookahead during token subdivision"
        );
        self.tokens
            .get(self.cursor + distance)
            .map_or(TokenKind::Eof, |token| token.kind())
    }

    fn current_kind(&self) -> TokenKind {
        self.split
            .map_or_else(|| self.tokens[self.cursor].kind(), SyntaxToken::kind)
    }

    fn current_span(&self) -> Span {
        self.split.map_or_else(
            || self.tokens[self.cursor].span(),
            |token| Span::new(self.source.id(), token.range()),
        )
    }

    fn current_text(&self) -> &str {
        self.source
            .text_at(self.current_span().range())
            .expect("token spans are UTF-8 boundaries in their source")
    }

    fn bump(&mut self) {
        if let Some(token) = self.split.take() {
            self.events.push(Event::Token(token));
            self.cursor += 1;
            return;
        }

        let lexical = TokenId::new(self.cursor);
        let token = self.tokens[self.cursor];
        self.events.push(Event::Token(SyntaxToken::new(
            self.source.id(),
            lexical,
            token.kind(),
            token.span().range(),
        )));
        if !self.at(TokenKind::Eof) {
            self.cursor += 1;
        }
    }

    fn split_current(&mut self, first: TokenKind, second: TokenKind) {
        assert!(
            self.split.is_none(),
            "cannot split an already subdivided token"
        );
        let token = self.tokens[self.cursor];
        let range = token.span().range();
        assert_eq!(
            range.len(),
            2,
            "two-way token subdivision requires two bytes"
        );
        let middle = nocter_source::ByteOffset::new(range.start().get() + 1);
        let lexical = TokenId::new(self.cursor);
        self.events.push(Event::Token(SyntaxToken::new(
            self.source.id(),
            lexical,
            first,
            TextRange::new(range.start(), middle),
        )));
        self.split = Some(SyntaxToken::new(
            self.source.id(),
            lexical,
            second,
            TextRange::new(middle, range.end()),
        ));
    }

    fn eat_type_greater(&mut self) -> bool {
        if self.eat_punctuation(Punctuation::Greater) {
            true
        } else if self.at_punctuation(Punctuation::ShiftRight) {
            self.split_current(
                TokenKind::Punctuation(Punctuation::Greater),
                TokenKind::Punctuation(Punctuation::Greater),
            );
            true
        } else {
            false
        }
    }

    fn expect_type_greater(&mut self) -> bool {
        if self.eat_type_greater() {
            true
        } else {
            self.missing(ExpectedSyntax::Punctuation(Punctuation::Greater));
            false
        }
    }

    fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        self.eat(TokenKind::Keyword(keyword))
    }

    fn eat_punctuation(&mut self, punctuation: Punctuation) -> bool {
        self.eat(TokenKind::Punctuation(punctuation))
    }

    fn expect(&mut self, kind: TokenKind) -> bool {
        if self.eat(kind) {
            true
        } else {
            self.missing(ExpectedSyntax::Token(kind));
            false
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> bool {
        if self.eat_keyword(keyword) {
            true
        } else {
            self.missing(ExpectedSyntax::Keyword(keyword));
            false
        }
    }

    fn expect_punctuation(&mut self, punctuation: Punctuation) -> bool {
        if self.eat_punctuation(punctuation) {
            true
        } else {
            self.missing(ExpectedSyntax::Punctuation(punctuation));
            false
        }
    }

    fn expect_name(&mut self) -> bool {
        if self.at(TokenKind::Identifier) && !matches!(self.current_text(), "_" | "Self") {
            self.bump();
            true
        } else if self.at(TokenKind::Identifier) {
            self.error_token(ExpectedSyntax::Name);
            false
        } else {
            self.missing(ExpectedSyntax::Name);
            false
        }
    }

    fn expect_name_or_discard(&mut self) -> bool {
        if self.at_identifier_text("_") {
            self.bump();
            true
        } else {
            self.expect_name()
        }
    }

    fn expect_identifier_text(&mut self, text: &'static str) -> bool {
        if self.at_identifier_text(text) {
            self.bump();
            true
        } else {
            self.missing(ExpectedSyntax::Contextual(text));
            false
        }
    }

    fn missing(&mut self, expected: ExpectedSyntax) {
        let span = self.empty_current_span();
        self.events.push(missing(expected, span));
        self.diagnostics.push(ParseDiagnostic::new(
            ParseDiagnosticKind::Expected(expected),
            span,
        ));
    }

    fn diagnostic(&mut self, kind: ParseDiagnosticKind) {
        self.diagnostics
            .push(ParseDiagnostic::new(kind, self.current_span()));
    }

    fn error_token(&mut self, expected: ExpectedSyntax) {
        if self.at(TokenKind::Eof) {
            self.missing(expected);
            return;
        }
        self.diagnostic(ParseDiagnosticKind::Expected(expected));
        let marker = self.start();
        self.bump();
        self.complete(marker, NodeKind::Error);
    }

    fn empty_current_span(&self) -> Span {
        let offset = self.current_span().range().start();
        self.source.span(TextRange::empty(offset))
    }

    fn eat_newlines(&mut self) -> usize {
        let mut count = 0;
        while self.eat(TokenKind::Newline) {
            count += 1;
        }
        count
    }

    fn comma_list(
        &mut self,
        closing: Punctuation,
        allow_empty: bool,
        expected_element: ExpectedSyntax,
        parse_element: fn(&mut Self),
    ) {
        self.eat_newlines();
        if self.at_punctuation(closing) || self.at(TokenKind::Eof) {
            if !allow_empty {
                self.missing(expected_element);
            }
            return;
        }

        loop {
            let before = self.cursor;
            parse_element(self);
            if self.cursor == before {
                self.error_token(expected_element);
            }

            if self.eat_punctuation(Punctuation::Comma) {
                self.eat_newlines();
                if self.at_punctuation(closing) || self.at(TokenKind::Eof) {
                    return;
                }
                continue;
            }

            self.eat_newlines();
            if self.at_punctuation(closing) || self.at(TokenKind::Eof) {
                return;
            }

            self.missing(ExpectedSyntax::Punctuation(Punctuation::Comma));
        }
    }

    fn line_sequence(
        &mut self,
        closing: Punctuation,
        expected_element: ExpectedSyntax,
        parse_element: fn(&mut Self),
    ) {
        self.eat_newlines();
        while !self.at_punctuation(closing) && !self.at(TokenKind::Eof) {
            let before = (self.cursor, self.split);
            parse_element(self);
            if (self.cursor, self.split) == before {
                self.error_token(expected_element);
            }

            if self.at_punctuation(closing) || self.at(TokenKind::Eof) {
                break;
            }
            if self.eat_newlines() == 0 {
                self.missing(ExpectedSyntax::Newline);
                self.recover_until(&[TokenKind::Newline, TokenKind::Punctuation(closing)]);
                self.eat_newlines();
            }
        }
    }

    fn braced_line_sequence(
        &mut self,
        expected_element: ExpectedSyntax,
        parse_element: fn(&mut Self),
    ) {
        if !self.expect_punctuation(Punctuation::LeftBrace) {
            return;
        }
        if !self.enter_nesting() {
            self.recover_balanced(Punctuation::LeftBrace, Punctuation::RightBrace);
            return;
        }
        self.line_sequence(Punctuation::RightBrace, expected_element, parse_element);
        self.expect_punctuation(Punctuation::RightBrace);
        self.leave_nesting();
    }

    fn require_line_end(&mut self) {
        if self.at(TokenKind::Eof) {
            return;
        }
        if self.eat_newlines() > 0 {
            return;
        }

        self.missing(ExpectedSyntax::Newline);
        self.recover_to_line_end();
        self.eat_newlines();
    }

    fn recover_to_line_end(&mut self) {
        if self.at(TokenKind::Eof) || self.at(TokenKind::Newline) {
            return;
        }
        let marker = self.start();
        while !self.at(TokenKind::Eof) && !self.at(TokenKind::Newline) {
            self.bump();
        }
        self.complete(marker, NodeKind::Error);
    }

    fn recover_until(&mut self, boundaries: &[TokenKind]) {
        if self.at(TokenKind::Eof) || boundaries.contains(&self.current_kind()) {
            return;
        }
        let marker = self.start();
        while !self.at(TokenKind::Eof) && !boundaries.contains(&self.current_kind()) {
            self.bump();
        }
        self.complete(marker, NodeKind::Error);
    }

    /// Recover after an opening delimiter has already been consumed.
    ///
    /// The matching closer is included in the error node so the caller must not consume it again.
    fn recover_balanced(&mut self, opening: Punctuation, closing: Punctuation) {
        let marker = self.start();
        let mut depth = 1_u32;
        while depth > 0 && !self.at(TokenKind::Eof) {
            if self.at_punctuation(opening) {
                depth += 1;
            } else if self.at_punctuation(closing) {
                depth -= 1;
            }
            self.bump();
        }
        self.complete(marker, NodeKind::Error);
    }

    fn enter_nesting(&mut self) -> bool {
        if self.nesting == Self::MAX_NESTING {
            self.diagnostic(ParseDiagnosticKind::NestingLimit);
            false
        } else {
            self.nesting += 1;
            true
        }
    }

    fn leave_nesting(&mut self) {
        self.nesting -= 1;
    }

    /// Parse one syntactically ambiguous branch transactionally.
    ///
    /// A successful branch keeps its events and is never parsed twice. A failed branch restores
    /// the token and event state. Safety-limit diagnostics survive rollback because they describe
    /// the input's structural depth rather than the rejected branch's ordinary syntax errors.
    fn attempt(&mut self, parse: fn(&mut Self)) -> bool {
        self.attempt_with(parse, |_, ()| true).is_some()
    }

    fn attempt_with<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> T,
        accept: impl FnOnce(&Self, &T) -> bool,
    ) -> Option<T> {
        let checkpoint = self.checkpoint();
        let value = parse(self);
        if self.diagnostics.len() == checkpoint.diagnostic_count && accept(self, &value) {
            return Some(value);
        }
        self.rollback(checkpoint);
        None
    }

    /// Parse a branch whose token discriminator can commit even when its interior is malformed.
    ///
    /// Diagnostics produced before the discriminator are retained only if `accept` confirms the
    /// branch. This differs from `attempt_with`, where any diagnostic rejects the branch.
    fn attempt_decided_with<T>(
        &mut self,
        parse: impl FnOnce(&mut Self) -> T,
        accept: impl FnOnce(&Self, &T) -> bool,
    ) -> Option<T> {
        let checkpoint = self.checkpoint();
        let value = parse(self);
        if accept(self, &value) {
            return Some(value);
        }
        self.rollback(checkpoint);
        None
    }

    fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            cursor: self.cursor,
            split: self.split,
            nesting: self.nesting,
            event_count: self.events.len(),
            diagnostic_count: self.diagnostics.len(),
        }
    }

    fn rollback(&mut self, checkpoint: Checkpoint) {
        let nesting_diagnostic = self.diagnostics[checkpoint.diagnostic_count..]
            .iter()
            .copied()
            .find(|diagnostic| diagnostic.kind() == ParseDiagnosticKind::NestingLimit);

        self.cursor = checkpoint.cursor;
        self.split = checkpoint.split;
        self.nesting = checkpoint.nesting;
        self.events.truncate(checkpoint.event_count);
        self.diagnostics.truncate(checkpoint.diagnostic_count);
        self.diagnostics.extend(nesting_diagnostic);
    }
}

#[derive(Clone, Copy)]
struct Marker {
    event_index: usize,
}

#[derive(Clone, Copy)]
struct CompletedMarker {
    event_index: usize,
}

#[derive(Clone, Copy)]
struct Checkpoint {
    cursor: usize,
    split: Option<SyntaxToken>,
    nesting: u16,
    event_count: usize,
    diagnostic_count: usize,
}
