use super::super::{FrontendOptions, load_compile_unit};
use crate::analysis::analyze_compile_unit_with_entry;
use crate::diagnostics::Diagnostic;
use crate::entry::DEFAULT_ENTRY_NAME;
use crate::source::{SourceId, SourceMap};
use crate::target::DEFAULT_TARGET;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn make_temp_project(name: &str) -> PathBuf {
    let unique = format!(
        "nocter-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let root = env::temp_dir().join(unique);
    fs::create_dir_all(&root).unwrap();
    root
}

pub(super) fn make_nocter_home(root: &Path) -> PathBuf {
    let home = root.join(".nocter");
    fs::create_dir_all(home.join("std")).unwrap();
    fs::write(home.join("std/prelude.nct"), "pub type Int = i32\n").unwrap();
    home
}

pub(super) fn check_with_nocter_home(
    sources: &mut SourceMap,
    source: SourceId,
    home: &Path,
) -> Vec<Diagnostic> {
    let unit = match load_compile_unit(
        sources,
        source,
        &FrontendOptions {
            nocter_home: Some(home.to_path_buf()),
            target: DEFAULT_TARGET.to_string(),
        },
    ) {
        Ok(unit) => unit,
        Err(diagnostics) => return diagnostics,
    };

    analyze_compile_unit_with_entry(sources, &unit, DEFAULT_ENTRY_NAME).diagnostics()
}
