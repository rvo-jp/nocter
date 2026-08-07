use super::*;

impl Parser<'_> {
    pub(crate) fn parse_result_provenance_clause(
        &mut self,
    ) -> ParseResult<Option<ResultProvenanceClause>> {
        let Some(from) = self.match_identifier_text("from") else {
            return Ok(None);
        };
        let mut origins = Vec::new();

        let end = loop {
            let origin = self.expect_identifier("expected result origin after `from`")?;
            let end = origin.span.end;
            let kind = match origin.value.as_str() {
                "self" => ResultProvenanceOriginKind::Receiver,
                "static" => ResultProvenanceOriginKind::Static,
                _ => ResultProvenanceOriginKind::Parameter(origin.value),
            };
            origins.push(ResultProvenanceOrigin {
                span: origin.span,
                kind,
            });

            if self.match_punctuation("|").is_none() {
                break end;
            }
        };

        Ok(Some(ResultProvenanceClause {
            span: self.span(from.span.start, end),
            origins,
        }))
    }
}
