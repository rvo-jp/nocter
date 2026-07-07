use super::FrontendOptions;
use super::imports::{active_nocter_home, canonicalize_existing};
use crate::ast::{AstFile, Item, ModulePath, UseItem};
use crate::source::{ByteSpan, SourceId, SourceMap};
use std::path::PathBuf;

const STANDARD_PRELUDE_PATH: &str = "std/prelude";

pub(super) fn should_synthesize_prelude(
    sources: &SourceMap,
    source: SourceId,
    ast: &AstFile,
    options: &FrontendOptions,
    resolved_nocter_home: &mut Option<Result<PathBuf, String>>,
) -> bool {
    if ast.items.iter().any(is_standard_prelude_use) {
        return false;
    }

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

pub(super) fn synthesize_prelude_use(source: SourceId, ast: &mut AstFile) {
    let span = ByteSpan::new(source, 0, 0);
    ast.items.insert(
        0,
        Item::Use(UseItem {
            span,
            path: ModulePath {
                span,
                value: STANDARD_PRELUDE_PATH.to_string(),
                segments: vec!["std".to_string(), "prelude".to_string()],
            },
        }),
    );
}

fn is_standard_prelude_use(item: &Item) -> bool {
    matches!(item, Item::Use(use_) if use_.path.value == STANDARD_PRELUDE_PATH)
}
