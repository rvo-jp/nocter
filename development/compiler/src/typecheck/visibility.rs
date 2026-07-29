use crate::ast::Visibility;
use crate::resolve::{ImportAccess, ResolveOutput};
use crate::source::{ByteSpan, SourceId};

pub(super) fn member_visibility_is_accessible(
    visibility: Visibility,
    declaration_span: ByteSpan,
    use_source: SourceId,
    resolved: &ResolveOutput,
) -> bool {
    match visibility {
        Visibility::Public => true,
        Visibility::Private => declaration_span.source == use_source,
        Visibility::Nocter => {
            declaration_span.source == use_source || resolved.access == ImportAccess::Nocter
        }
    }
}
