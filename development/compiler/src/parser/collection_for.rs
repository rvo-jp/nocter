use super::support::ParsedIdentifier;
use super::{ParseResult, Parser};
use crate::ast::{CollectionForStmt, Expr, LiteralPackForStmt, Stmt};
use crate::lexer::Token;

impl Parser<'_> {
    pub(super) fn finish_non_range_for_statement(
        &mut self,
        start: Token,
        name: ParsedIdentifier,
        source: Expr,
    ) -> ParseResult<Stmt> {
        let body = self.parse_block()?;
        if let Expr::Identifier(pack) = &source
            && self.literal_pack_capture.as_deref() == Some(pack.name.as_str())
        {
            return Ok(Stmt::LiteralPackFor(LiteralPackForStmt {
                span: self.span(start.span.start, body.span.end),
                name: name.value,
                name_span: name.span,
                pack_name: pack.name.clone(),
                pack_span: pack.span,
                body,
            }));
        }

        Ok(Stmt::CollectionFor(CollectionForStmt {
            span: self.span(start.span.start, body.span.end),
            name: name.value,
            name_span: name.span,
            source,
            body,
        }))
    }
}
