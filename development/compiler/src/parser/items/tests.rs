use super::*;
use crate::ast::TestDecl;

impl Parser<'_> {
    pub(super) fn parse_test_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Test, "`test`")?;
        let name = self.expect_name_identifier("expected test name after `test`")?;
        let body = self.parse_block()?;
        Ok(Item::Test(TestDecl {
            span: self.span(start.span.start, body.span.end),
            name: name.value,
            name_span: name.span,
            body,
        }))
    }
}
