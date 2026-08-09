use super::{ParseResult, Parser};
use crate::ast::{CallableGenericRequirement, CallableRequirementClause, TypeExpr};

impl Parser<'_> {
    pub(super) fn parse_callable_requirement_clause(
        &mut self,
    ) -> ParseResult<Option<CallableRequirementClause>> {
        let Some(keyword) = self.match_identifier_text("where") else {
            return Ok(None);
        };
        let mut requirements = Vec::new();
        loop {
            let copy_span = self.match_identifier_text("copy").map(|token| token.span);
            let name = self.expect_name_identifier("expected generic parameter after `where`")?;
            let mut bounds = Vec::<TypeExpr>::new();
            if self.match_punctuation(":").is_some() {
                loop {
                    bounds.push(self.parse_type()?);
                    if self.match_punctuation("+").is_none() {
                        break;
                    }
                }
            } else if copy_span.is_none() {
                self.error_at(
                    name.span,
                    "a callable requirement must contain `copy` or `:` bounds",
                );
                return Err(());
            }
            let end = bounds
                .last()
                .map_or(name.span.end, |bound| bound.span().end);
            requirements.push(CallableGenericRequirement {
                span: self.span(copy_span.map_or(name.span.start, |span| span.start), end),
                copy_span,
                name: name.value,
                name_span: name.span,
                bounds,
            });
            if self.match_punctuation(",").is_none() {
                break;
            }
        }
        let end = requirements
            .last()
            .map_or(keyword.span.end, |requirement| requirement.span.end);
        Ok(Some(CallableRequirementClause {
            span: self.span(keyword.span.start, end),
            keyword_span: keyword.span,
            requirements,
        }))
    }
}
