use super::{ParseResult, Parser};
use crate::ast::{ConstructDecl, ConstructMember, ConstructMemberDecl, FunctionOwner, Item};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_construct_decl(&mut self) -> ParseResult<Item> {
        let keyword = self.expect_keyword(Keyword::Construct, "`construct`")?;
        let target = self.parse_type()?;
        let (target_name, target_name_span) = super::items::type_owners::owner_target_name(&target)
            .ok_or_else(|| {
                self.error_at(
                    target.span(),
                    "construct target must be a nominal type reference",
                );
            })?;
        let owner_generics =
            super::items::type_owners::owner_target_generics(&target).map_err(|()| {
                self.error_at(
                    target.span(),
                    "construct target arguments must be generic parameter names",
                );
            })?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut members = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close construct declaration");
                return Err(());
            }

            let member_start = self.current().span.start;
            let visibility = self.parse_visibility()?;
            let default_span = self
                .match_identifier_text("default")
                .map(|token| token.span);
            self.reject_removed_result_allocation_modifier()?;

            let declaration = if self.at_keyword(Keyword::Func) {
                let mut function = self.parse_function_decl_data(visibility, None)?;
                if function.owner.is_some() {
                    self.error_at(
                        function.name_span,
                        "construct function names are unqualified; the construct target supplies the owner",
                    );
                    return Err(());
                }
                function.owner = Some(FunctionOwner {
                    name: target_name.clone(),
                    name_span: target_name_span,
                });
                function.name = format!("{}.{}", target_name, function.member_name);
                function
                    .generics
                    .parameters
                    .splice(0..0, owner_generics.parameters.iter().cloned());
                ConstructMemberDecl::Function(function)
            } else if self.at_keyword(Keyword::Literal) {
                ConstructMemberDecl::Literal(
                    self.parse_literal_decl_data(visibility, Some(target.clone()))?,
                )
            } else {
                self.error_current("expected `func` or `literal` in construct declaration");
                return Err(());
            };

            let end = match &declaration {
                ConstructMemberDecl::Function(function) => function.span.end,
                ConstructMemberDecl::Literal(literal) => literal.span.end,
            };
            members.push(ConstructMember {
                span: self.span(member_start, end),
                default_span,
                declaration,
            });
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Construct(ConstructDecl {
            span: self.span(keyword.span.start, close.span.end),
            keyword_span: keyword.span,
            target,
            members,
        }))
    }
}
