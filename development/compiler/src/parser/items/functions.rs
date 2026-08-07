use super::*;

impl Parser<'_> {
    pub(super) fn parse_function_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
        result_allocation: Option<ResultAllocationModifier>,
    ) -> ParseResult<Item> {
        self.parse_function_decl_data(visibility, target, result_allocation)
            .map(Item::Function)
    }

    pub(in crate::parser) fn parse_function_decl_data(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
        result_allocation: Option<ResultAllocationModifier>,
    ) -> ParseResult<FunctionDecl> {
        let start = self.expect_keyword(Keyword::Func, "`func`")?;
        let first_name = self.expect_name_identifier("expected function name after `func`")?;
        let (owner, name, name_span, member_name, member_name_span) =
            if self.match_punctuation(".").is_some() {
                let member =
                    self.expect_name_identifier("expected associated function name after `.`")?;
                (
                    Some(FunctionOwner {
                        name: first_name.value.clone(),
                        name_span: first_name.span,
                    }),
                    format!("{}.{}", first_name.value, member.value),
                    self.span(first_name.span.start, member.span.end),
                    member.value,
                    member.span,
                )
            } else {
                (
                    None,
                    first_name.value.clone(),
                    first_name.span,
                    first_name.value,
                    first_name.span,
                )
            };
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let body = self.parse_block()?;
        let end = body.span.end;

        Ok(FunctionDecl {
            span: self.span(
                target.as_ref().map_or_else(
                    || result_allocation.map_or(start.span.start, |modifier| modifier.span.start),
                    |target| target.span.start,
                ),
                end,
            ),
            visibility,
            target,
            keyword_span: start.span,
            result_allocation,
            owner,
            name,
            name_span,
            member_name,
            member_name_span,
            generics,
            parameters,
            return_type,
            result_provenance,
            body,
        })
    }

    pub(super) fn parse_primitive_decl(
        &mut self,
        visibility: Visibility,
        target: Option<TargetDirective>,
        result_allocation: Option<ResultAllocationModifier>,
    ) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Primitive, "`primitive`")?;
        let name = self.expect_name_identifier("expected primitive name after `primitive`")?;
        let generics = self.parse_generic_param_list()?;
        let parameters = self.parse_parameter_list()?;
        self.expect_punctuation(":", "`:`")?;
        let return_type = self.parse_type()?;
        let result_provenance = self.parse_result_provenance_clause()?;
        let end = result_provenance
            .as_ref()
            .map_or(return_type.span().end, |clause| clause.span.end);

        Ok(Item::Primitive(PrimitiveDecl {
            span: self.span(
                target.as_ref().map_or_else(
                    || result_allocation.map_or(start.span.start, |modifier| modifier.span.start),
                    |target| target.span.start,
                ),
                end,
            ),
            visibility,
            target,
            keyword_span: start.span,
            result_allocation,
            name: name.value,
            name_span: name.span,
            generics,
            parameters,
            return_type,
            result_provenance,
        }))
    }
}
