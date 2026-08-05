use crate::source::SourceId;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Default)]
pub(super) struct SourceDependencyTrace {
    sources: HashSet<SourceId>,
    paths: HashSet<PathBuf>,
}

impl SourceDependencyTrace {
    pub(super) fn record_source(&mut self, source: SourceId) -> bool {
        self.sources.insert(source)
    }

    pub(super) fn record_path(&mut self, path: impl AsRef<Path>) {
        self.paths.extend(dependency_path_aliases(path));
    }

    pub(super) fn record_module_candidates(&mut self, module_path: &Path) {
        self.record_path(module_path.with_extension("nct"));
        self.record_path(module_path.join("index.nct"));
    }

    pub(super) fn into_parts(self) -> (HashSet<SourceId>, HashSet<PathBuf>) {
        (self.sources, self.paths)
    }
}

pub(crate) fn dependency_path_aliases(path: impl AsRef<Path>) -> HashSet<PathBuf> {
    let path = path.as_ref();
    let mut aliases = HashSet::from([normalize_lexically(path)]);
    if let Ok(canonical) = path.canonicalize() {
        aliases.insert(canonical);
    }
    if let Some(canonical) = canonicalize_existing_ancestor(path) {
        aliases.insert(canonical);
    }
    aliases
}

fn canonicalize_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut ancestor = path;
    let mut missing_tail = Vec::<OsString>::new();
    loop {
        if let Ok(mut canonical) = ancestor.canonicalize() {
            for component in missing_tail.iter().rev() {
                canonical.push(component);
            }
            return Some(canonical);
        }
        missing_tail.push(ancestor.file_name()?.to_os_string());
        ancestor = ancestor.parent()?;
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    normalized.pop();
                }
                Some(Component::ParentDir) | None if !path.is_absolute() => {
                    normalized.push(component);
                }
                _ => {}
            },
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexical_alias_survives_for_a_missing_path() {
        let aliases = dependency_path_aliases(Path::new("/tmp/nocter/a/../missing.nct"));

        assert!(aliases.contains(Path::new("/tmp/nocter/missing.nct")));
    }

    #[test]
    fn lexical_normalization_preserves_leading_relative_parents() {
        assert_eq!(
            normalize_lexically(Path::new("../../package/file.nct")),
            Path::new("../../package/file.nct")
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_leaf_retains_the_canonical_existing_ancestor() {
        use std::os::unix::fs::symlink;

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nocter-dependency-alias-{}-{stamp}",
            std::process::id()
        ));
        let real = root.join("real");
        let alias = root.join("alias");
        std::fs::create_dir_all(&real).unwrap();
        symlink(&real, &alias).unwrap();

        let aliases = dependency_path_aliases(alias.join("missing.nct"));
        assert!(aliases.contains(&real.canonicalize().unwrap().join("missing.nct")));

        std::fs::remove_dir_all(&root).unwrap();
    }
}
