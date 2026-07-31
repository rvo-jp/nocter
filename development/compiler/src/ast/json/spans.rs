use super::*;

pub(super) fn json_span(sources: &SourceMap, span: ByteSpan) -> Option<JsonSpan> {
    sources.span_to_json(span).ok()
}
