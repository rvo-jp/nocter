use super::{ParseResult, Parser};
use crate::ast::{RegionStmt, Stmt};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_region_statement(&mut self) -> ParseResult<Stmt> {
        let keyword = self.expect_keyword(Keyword::Region, "`region`")?;
        let name = self.expect_name_identifier("expected region binding name")?;
        let using = self.expect_keyword(Keyword::Using, "`using`")?;
        let allocator = self.parse_expression()?;
        let body = self.parse_block()?;

        Ok(Stmt::Region(RegionStmt {
            span: self.span(keyword.span.start, body.span.end),
            keyword_span: keyword.span,
            name: name.value,
            name_span: name.span,
            using_span: using.span,
            allocator,
            body,
        }))
    }
}
