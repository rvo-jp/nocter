use super::Parser;
use crate::ast::ResultAllocationModifier;

impl Parser<'_> {
    pub(super) fn parse_optional_result_allocation_modifier(
        &mut self,
    ) -> Option<ResultAllocationModifier> {
        self.match_identifier_text("alloc")
            .map(|token| ResultAllocationModifier { span: token.span })
    }
}
