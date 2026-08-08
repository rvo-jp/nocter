use super::super::{FrontendOptions, load_compile_unit};
use crate::analysis::analyze_executable_compile_unit;
use crate::diagnostics::Diagnostic;
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
    fs::create_dir_all(home.join("std/prelude")).unwrap();
    fs::create_dir_all(home.join("std/str")).unwrap();
    fs::create_dir_all(home.join("std/slice")).unwrap();
    crate::test_files::write(home.join("std/prelude/index.nct"), "").unwrap();
    crate::test_files::write(
        home.join("std/str/index.nct"),
        r#"impl str {
    pub method &self.len(): usize { return 0 }
    pub method &self.is_empty(): bool { return self.len() == 0 }
}
"#,
    )
    .unwrap();
    crate::test_files::write(
        home.join("std/slice/index.nct"),
        r#"impl<T> [T] {
    pub method &self.len(): usize { return 0 }
    pub method &self.is_empty(): bool { return self.len() == 0 }
}
"#,
    )
    .unwrap();
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
            package_graph: None,
            target: DEFAULT_TARGET.to_string(),
        },
    ) {
        Ok(unit) => unit,
        Err(diagnostics) => return diagnostics,
    };

    analyze_executable_compile_unit(sources, &unit).diagnostics()
}
