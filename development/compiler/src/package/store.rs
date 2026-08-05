use super::PackageId;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(super) struct PackageStore {
    local: PathBuf,
    shared: Option<PathBuf>,
}

impl PackageStore {
    pub(super) fn new(package_root: &Path) -> Self {
        let shared = crate::home::resolve_nocter_home()
            .ok()
            .map(|home| home.join("packages"));
        Self {
            local: package_root.join(".nocter/packages"),
            shared,
        }
    }

    pub(super) fn find(&self, id: &PackageId) -> Option<PathBuf> {
        let local = self.local.join(id.as_str());
        if local.join("nocter.nct").is_file() {
            return Some(local);
        }
        let shared = self.shared.as_ref()?.join(id.as_str());
        shared.join("nocter.nct").is_file().then_some(shared)
    }

    pub(super) fn local_root(&self) -> &Path {
        &self.local
    }
}
