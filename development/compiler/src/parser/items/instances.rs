use super::*;

impl Parser<'_> {
    pub(super) fn parse_instance_decl(&mut self) -> ParseResult<Item> {
        let start = self.expect_keyword(Keyword::Instance, "`instance`")?;
        self.reject_declaration_pattern_prefix("instance")?;
        let target_ty = self.parse_type()?;
        let generics = self.declaration_pattern_parameters(&[&target_ty])?;
        let mut requirements = self.parse_where_clause()?;
        self.classify_declaration_pattern_refinements(&mut requirements, &generics);
        let open = self.expect_punctuation("{", "`{`")?;
        let mut methods = Vec::new();
        let mut operators = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close instance block");
                return Err(());
            }
            let visibility = self.parse_visibility()?;
            self.reject_removed_result_allocation_modifier()?;
            if self.at_keyword(Keyword::Func) {
                self.error_current(
                    "associated `func` declarations are written at top level as `func Type.name(...)`",
                );
                return Err(());
            } else if self.at_keyword(Keyword::Method) {
                methods.push(self.parse_method_decl(visibility, true)?);
            } else if self.at_keyword(Keyword::Operator) {
                operators.push(self.parse_operator_decl(visibility)?);
            } else if self.at_identifier_text("drop") {
                self.error_current(
                    "drop members were removed; write `destruct Type(&+self) { ... }` at top level",
                );
                return Err(());
            } else {
                self.error_current("expected `method` or `operator` in instance block");
                return Err(());
            }
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Instance(InstanceDecl {
            span: self.span(start.span.start, close.span.end),
            generics,
            target_ty,
            requirements,
            methods,
            operators,
        }))
    }
}
