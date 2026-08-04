use super::{ParseResult, Parser};
use crate::ast::{
    ConstructDecl, ConstructMember, ConstructMemberDecl, FunctionOwner, GenericParam,
    GenericParamList, GenericType, Item, TypeExpr, Visibility,
};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_construct_decl(&mut self) -> ParseResult<Item> {
        let keyword = self.expect_keyword(Keyword::Construct, "`construct`")?;
        let target = self.parse_type()?;
        let (target_name, target_name_span) = construct_target_name(&target).ok_or_else(|| {
            self.error_at(
                target.span(),
                "construct target must be a nominal type reference",
            );
        })?;
        let owner_generics = construct_target_generics(&target)?;
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
            if visibility != Visibility::Public {
                self.error_current("construct members must be explicitly marked `pub`");
                return Err(());
            }
            let default_span = self
                .match_identifier_text("default")
                .map(|token| token.span);

            let declaration = if self.at_keyword(Keyword::Func) {
                let mut function = self.parse_function_decl_data(Visibility::Public, None)?;
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
                    self.parse_literal_decl_data(Visibility::Public, Some(target.clone()))?,
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

fn construct_target_name(target: &TypeExpr) -> Option<(String, crate::source::ByteSpan)> {
    match target {
        TypeExpr::Reference(reference) => Some((reference.name.clone(), reference.span)),
        TypeExpr::Generic(generic) => Some((generic.name.clone(), generic.name_span)),
        _ => None,
    }
}

fn construct_target_generics(target: &TypeExpr) -> ParseResult<GenericParamList> {
    let TypeExpr::Generic(GenericType { arguments, .. }) = target else {
        return Ok(GenericParamList::empty());
    };
    let mut parameters = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let TypeExpr::Reference(reference) = argument else {
            return Err(());
        };
        parameters.push(GenericParam {
            span: reference.span,
            name: reference.name.clone(),
            name_span: reference.span,
            bounds: Vec::new(),
        });
    }
    Ok(GenericParamList {
        span: Some(target.span()),
        parameters,
    })
}
