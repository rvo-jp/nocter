use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub(super) struct TemporaryExecutable {
    root: PathBuf,
    executable: PathBuf,
}

impl TemporaryExecutable {
    pub(super) fn new(purpose: &str) -> io::Result<Self> {
        let root = std::env::temp_dir().join(unique_directory_name(purpose));
        fs::create_dir_all(&root)?;
        let executable = root.join("app");
        Ok(Self { root, executable })
    }

    pub(super) fn path(&self) -> &Path {
        &self.executable
    }
}

impl Drop for TemporaryExecutable {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.executable);
        let _ = fs::remove_dir(&self.root);
    }
}

fn unique_directory_name(purpose: &str) -> String {
    format!(
        "nocter-{purpose}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_and_removes_one_unique_executable_directory() {
        let root;
        {
            let artifact = TemporaryExecutable::new("test").unwrap();
            root = artifact.root.clone();
            assert!(artifact.path().ends_with("app"));
            assert!(root.is_dir());
        }
        assert!(!root.exists());
    }
}
