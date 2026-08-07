use super::{ParseResult, Parser};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn reject_removed_result_allocation_modifier(&mut self) -> ParseResult<()> {
        if !self.at_identifier_text("alloc") {
            return Ok(());
        }

        let followed_by_callable = self.token_at_offset_is_keyword(1, Keyword::Func)
            || self.token_at_offset_is_keyword(1, Keyword::Method)
            || self.token_at_offset_is_keyword(1, Keyword::Primitive)
            || self.token_at_offset_is_keyword(1, Keyword::Literal)
            || ((self.punctuation_at_offset(1, "&") || self.punctuation_at_offset(1, "&+"))
                && self.token_at_offset_is_keyword(2, Keyword::Func));
        if !followed_by_callable {
            return Ok(());
        }

        self.error_current(
            "result `alloc` modifiers have been removed; allocation is inferred and only caller-managed result origins use `from`",
        );
        Err(())
    }
}
