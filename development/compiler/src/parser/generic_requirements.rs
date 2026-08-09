use super::{ParseResult, Parser};
use crate::ast::{
    GenericRequirementPredicate, TypeEqualityPredicate, TypeExpr, WhereClause, WherePredicate,
};

impl Parser<'_> {
    pub(super) fn parse_where_clause(&mut self) -> ParseResult<Option<WhereClause>> {
        let Some(keyword) = self.match_identifier_text("where") else {
            return Ok(None);
        };
        let mut predicates = Vec::new();
        loop {
            if let Some(copy_token) = self.match_identifier_text("copy") {
                let name =
                    self.expect_name_identifier("expected generic parameter after `where copy`")?;
                let bounds = self.parse_optional_where_bounds()?;
                let end = bounds
                    .last()
                    .map_or(name.span.end, |bound| bound.span().end);
                predicates.push(WherePredicate::Generic(GenericRequirementPredicate {
                    span: self.span(copy_token.span.start, end),
                    copy_span: Some(copy_token.span),
                    name: name.value,
                    name_span: name.span,
                    bounds,
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
                        self.error_at(
                            reference.span,
                            "a where predicate must contain `:`, `=`, or prefix `copy`",
                        );
                        return Err(());
                    }
                    let bounds = self.parse_required_where_bounds()?;
                    let end = bounds
                        .last()
                        .map_or(reference.span.end, |bound| bound.span().end);
                    predicates.push(WherePredicate::Generic(GenericRequirementPredicate {
                        span: self.span(reference.span.start, end),
                        copy_span: None,
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
                WherePredicate::Generic(requirement) => requirement.span.end,
                WherePredicate::Equality(equality) => equality.span.end,
            });
        Ok(Some(WhereClause {
            span: self.span(keyword.span.start, end),
            keyword_span: keyword.span,
            predicates,
        }))
    }

    fn parse_optional_where_bounds(&mut self) -> ParseResult<Vec<TypeExpr>> {
        if self.match_punctuation(":").is_none() {
            return Ok(Vec::new());
        }
        self.parse_required_where_bounds()
    }

    fn parse_required_where_bounds(&mut self) -> ParseResult<Vec<TypeExpr>> {
        let mut bounds = Vec::new();
        loop {
            bounds.push(self.parse_type()?);
            if self.match_punctuation("+").is_none() {
                break;
            }
        }
        Ok(bounds)
    }
}
