use crate::ast::Visibility;
use crate::resolve::ResolveOutput;
use crate::source::{ByteSpan, SourceId};

pub(super) fn member_visibility_is_accessible(
    visibility: Visibility,
    declaration_span: ByteSpan,
    use_source: SourceId,
    resolved: &ResolveOutput,
) -> bool {
    resolved.visibility_is_accessible(visibility, declaration_span.source, use_source)
}
