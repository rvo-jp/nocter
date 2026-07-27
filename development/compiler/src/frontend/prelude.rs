use super::FrontendOptions;
use super::imports::{active_nocter_home, canonicalize_existing};
use crate::ast::ModulePath;
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::path::PathBuf;

const STANDARD_PRELUDE_PATH: &str = "std/prelude";

pub(super) fn should_load_prelude(
    sources: &SourceMap,
    source: SourceId,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> bool {
    let Ok(home) = active_nocter_home(options, resolved_nocter_home) else {
        return true;
    };
    let home = canonicalize_existing(&home);
    let Some(source_path) = sources
        .get(source)
        .and_then(|file| file.absolute_path())
        .map(|path| canonicalize_existing(path))
    else {
        return true;
    };

    !source_path.starts_with(home)
}

pub(super) fn standard_prelude_path(source: SourceId) -> ModulePath {
    let span = ByteSpan::new(source, 0, 0);
    ModulePath {
        span,
        value: STANDARD_PRELUDE_PATH.to_string(),
        segments: vec!["std".to_string(), "prelude".to_string()],
        segment_spans: vec![span, span],
    }
}
