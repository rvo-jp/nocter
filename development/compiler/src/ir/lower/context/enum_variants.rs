use super::drop_glue::{
    enum_variant_index, payload_enum_symbol_for_type_expr, payloadless_enum_symbol_for_type_expr,
};
use super::*;

impl<'a> LoweringContext<'a> {
    pub(in crate::ir::lower) fn payloadless_enum_variant_tag(
        &self,
        member: &MemberExpr,
    ) -> Option<u8> {
        let symbol = self.enum_symbol_for_member(member)?;
        if symbol
            .variants
            .iter()
            .any(|variant| !variant.payload.is_empty())
        {
            return None;
        }
        enum_variant_index(symbol, &member.member)
    }

    pub(in crate::ir::lower) fn enum_variant_tag(&self, member: &MemberExpr) -> Option<u8> {
        enum_variant_index(self.enum_symbol_for_member(member)?, &member.member)
    }

    pub(in crate::ir::lower) fn enum_variant_payload_len(
        &self,
        member: &MemberExpr,
    ) -> Option<usize> {
        let symbol = self.enum_symbol_for_member(member)?;
        symbol
            .variants
            .iter()
            .find(|variant| variant.name == member.member)
            .map(|variant| variant.payload.len())
    }

    fn enum_symbol_for_member(&self, member: &MemberExpr) -> Option<&'a TypeSymbol> {
        let resolution = self.call_resolution.as_ref()?;
        let Expr::Identifier(enum_name) = member.object.as_ref() else {
            return None;
        };
        resolution
            .resolved
            .type_symbol_by_name(&enum_name.name)
            .filter(|symbol| symbol.kind == TypeSymbolKind::Enum)
    }

    pub(in crate::ir::lower) fn payloadless_enum_variant_names_for_expression(
        &self,
        expression: &Expr,
    ) -> Option<Vec<String>> {
        let resolution = self.call_resolution.as_ref()?;
        let ty = self.expression_type_expr(expression.span())?;
        let symbol = payloadless_enum_symbol_for_type_expr(&ty, resolution.resolved, &|source| {
            self.resolved_source(source)
        })?;
        Some(
            symbol
                .variants
                .iter()
                .map(|variant| variant.name.clone())
                .collect(),
        )
    }

    pub(in crate::ir::lower) fn payload_enum_variant_names_for_expression(
        &self,
        expression: &Expr,
    ) -> Option<Vec<String>> {
        let resolution = self.call_resolution.as_ref()?;
        let ty = self.expression_type_expr(expression.span())?;
        let symbol = payload_enum_symbol_for_type_expr(&ty, resolution.resolved, &|source| {
            self.resolved_source(source)
        })?;
        Some(
            symbol
                .variants
                .iter()
                .map(|variant| variant.name.clone())
                .collect(),
        )
    }
}
