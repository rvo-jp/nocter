use super::{ParseResult, Parser};
use crate::ast::{
    CopyRequirementPredicate, GenericRequirementPredicate, TypeEqualityPredicate, TypeExpr,
    WhereClause, WherePredicate,
};

impl Parser<'_> {
    pub(super) fn parse_where_clause(&mut self) -> ParseResult<Option<WhereClause>> {
        let Some(keyword) = self.match_identifier_text("where") else {
            return Ok(None);
        };
        let mut predicates = Vec::new();
        loop {
            if let Some(keyword) = self.match_identifier_text("copy") {
                let name =
                    self.expect_name_identifier("expected generic parameter after `copy`")?;
                if let Some(colon) = self.match_punctuation(":") {
                    self.error_at(
                        colon.span,
                        "`copy` is a distinct predicate; write `where copy T, T: Interface`",
                    );
                    return Err(());
                }
                predicates.push(WherePredicate::Copy(CopyRequirementPredicate {
                    span: self.span(keyword.span.start, name.span.end),
                    keyword_span: keyword.span,
                    name: name.value,
                    name_span: name.span,
                }));
            } else {
                let left = self.parse_type()?;
                if let Some(equals) = self.match_punctuation("=") {
                    let right = self.parse_type()?;
                    predicates.push(WherePredicate::Equality(TypeEqualityPredicate {
                        span: self.span(left.span().start, right.span().end),
                        equals_span: equals.span,
                        left,
                        right,
                    }));
                } else {
                    let TypeExpr::Reference(reference) = left else {
                        self.error_current(
                            "a generic requirement must name one parameter before `:`",
                        );
                        return Err(());
                    };
                    if self.match_punctuation(":").is_none() {
                        self.error_at(reference.span, "a where predicate must contain `:` or `=`");
                        return Err(());
                    }
                    let bounds = self.parse_required_where_bounds()?;
                    let end = bounds
                        .last()
                        .map_or(reference.span.end, |bound| bound.span().end);
                    predicates.push(WherePredicate::Generic(GenericRequirementPredicate {
                        span: self.span(reference.span.start, end),
                        name: reference.name,
                        name_span: reference.span,
                        bounds,
                    }));
                }
            }
            if self.match_punctuation(",").is_none() {
                break;
            }
        }
        let end = predicates
            .last()
            .map_or(keyword.span.end, |predicate| match predicate {
                WherePredicate::Copy(requirement) => requirement.span.end,
                WherePredicate::Generic(requirement) => requirement.span.end,
                WherePredicate::Equality(equality) => equality.span.end,
            });
        Ok(Some(WhereClause {
            span: self.span(keyword.span.start, end),
            keyword_span: keyword.span,
            predicates,
        }))
    }

    fn parse_required_where_bounds(&mut self) -> ParseResult<Vec<TypeExpr>> {
        let mut bounds = Vec::new();
        loop {
            if self.at_identifier_text("copy") {
                self.error_current("`copy` uses a prefix predicate; write `where copy T`");
                return Err(());
            }
            bounds.push(self.parse_type()?);
            if self.match_punctuation("+").is_none() {
                break;
            }
        }
        Ok(bounds)
    }
}
