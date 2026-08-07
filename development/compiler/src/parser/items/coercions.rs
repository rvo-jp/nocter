use super::*;
use crate::ast::{CoerceDecl, CoercionEntry, Item, MethodReceiverMode, Visibility};
use crate::lexer::Keyword;

impl Parser<'_> {
    pub(super) fn parse_coerce_decl(&mut self) -> ParseResult<Item> {
        let keyword = self.expect_keyword(Keyword::Coerce, "`coerce`")?;
        let target = self.parse_type()?;
        if !matches!(target, TypeExpr::Reference(_) | TypeExpr::Generic(_)) {
            self.error_at(
                target.span(),
                "coerce source must be a nominal type reference",
            );
            return Err(());
        }
        let generics = super::type_owners::owner_target_generics(&target).map_err(|()| {
            self.error_at(
                target.span(),
                "coerce source arguments must be generic parameter names",
            );
        })?;
        let open = self.expect_punctuation("{", "`{`")?;
        let mut entries = Vec::new();
        self.skip_newlines();

        while !self.at_punctuation("}") {
            if self.at_eof() {
                self.error_at(open.span, "expected `}` to close coerce declaration");
                return Err(());
            }

            let entry_start = self.current().span.start;
            let visibility = self.parse_visibility()?;
            if visibility == Visibility::Nocter {
                self.error_current("coercion entries cannot use `pub(nocter)`");
                return Err(());
            }
            let receiver = self.parse_coercion_receiver()?;
            let as_token = self.expect_keyword(Keyword::As, "`as`")?;
            let entry_target = self.parse_type()?;
            let result_provenance = self.parse_result_provenance_clause()?;
            let body = self.parse_block()?;
            entries.push(CoercionEntry {
                span: self.span(entry_start, body.span.end),
                visibility,
                receiver,
                as_span: as_token.span,
                target: entry_target,
                result_provenance,
                body,
            });
            self.skip_newlines();
        }

        let close = self.expect_punctuation("}", "`}`")?;
        Ok(Item::Coerce(CoerceDecl {
            span: self.span(keyword.span.start, close.span.end),
            keyword_span: keyword.span,
            target,
            generics,
            entries,
        }))
    }

    fn parse_coercion_receiver(&mut self) -> ParseResult<MethodReceiver> {
        let receiver =
            self.parse_self_receiver("expected `&self` or `&+self` receiver in coercion entry")?;
        if receiver.mode == MethodReceiverMode::Owned {
            self.error_at(
                receiver.span,
                "coercion receiver must be borrowed; write `&self` or `&+self`",
            );
            return Err(());
        }
        Ok(receiver)
    }
}
