use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::bstr::ByteSlice;
use gix::object::tree::EntryKind;
use nocter_package::{ExactDependencyLock, ExactDependencyLockKind};

use crate::PackageAcquisitionError;
use crate::http::install_crypto_provider;
use crate::policy::public_https_url;

const MAX_ENTRIES: u64 = 100_000;
const MAX_MATERIALIZED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PATH_COMPONENTS: usize = 64;
const LFS_POINTER_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/v1\n";

pub(crate) fn clone_repository(
    authored_url: &str,
    destination: &Path,
) -> Result<(), PackageAcquisitionError> {
    install_crypto_provider();
    let url = public_https_url(authored_url, false)?;
    let open = gix::open::Options::isolated().config_overrides([
        "http.followRedirects=false",
        "http.proxy=",
        "http.sslVerify=true",
    ]);
    let mut preparation = gix::clone::PrepareFetch::new(
        url.as_str(),
        destination,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
        open,
    )
    .map_err(|error| PackageAcquisitionError::invalid_git("prepare Git acquisition", error))?
    .configure_connection(|connection| {
        connection.set_credentials(reject_credentials);
        Ok(())
    });
    let interrupted = AtomicBool::new(false);
    preparation
        .fetch_only(gix::progress::Discard, &interrupted)
        .map_err(|error| {
            PackageAcquisitionError::invalid_git("fetch public Git repository", error)
        })?;
    Ok(())
}

#[expect(
    clippy::result_large_err,
    reason = "gix defines the credential protocol result; rejecting authentication needs its error"
)]
fn reject_credentials(
    _action: gix::credentials::helper::Action,
) -> gix::credentials::protocol::Result {
    Err(gix::credentials::protocol::Error::Quit)
}

pub(crate) fn resolve_revision(
    repository: &Path,
    revision: &str,
) -> Result<ExactDependencyLock, PackageAcquisitionError> {
    let repo = open_repository(repository)?;
    let id = if is_commit_id(revision) {
        commit_from_hex(&repo, revision)?
    } else if let Some(branch) = revision.strip_prefix("refs/heads/") {
        required_reference(&repo, &format!("refs/remotes/origin/{branch}"), revision)?
    } else if revision.starts_with("refs/tags/") {
        required_reference(&repo, revision, revision)?
    } else if revision.starts_with("refs/") {
        return Err(PackageAcquisitionError::invalid_git(
            "resolve Git revision",
            "only refs/heads/... and refs/tags/... are supported",
        ));
    } else {
        short_revision(&repo, revision)?
    };
    ExactDependencyLock::git(&id.to_string())
        .map_err(|error| PackageAcquisitionError::invalid_git("form exact Git lock", error))
}

pub(crate) fn export_commit(
    repository: &Path,
    lock: &ExactDependencyLock,
    destination: &Path,
) -> Result<(), PackageAcquisitionError> {
    if lock.kind() != ExactDependencyLockKind::Git {
        return Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "Git source requires a Git commit lock",
        ));
    }
    require_empty_directory(destination)?;
    let repo = open_repository(repository)?;
    let commit_id = commit_from_hex(&repo, lock.value())?;
    let commit = repo
        .find_commit(commit_id)
        .map_err(|error| PackageAcquisitionError::invalid_git("read locked Git commit", error))?;
    let tree = commit
        .tree()
        .map_err(|error| PackageAcquisitionError::invalid_git("read locked Git tree", error))?;
    let mut budget = MaterializationBudget::default();
    export_tree(&tree, destination, Path::new(""), 0, &mut budget)?;
    require_manifest(destination)
}

fn open_repository(path: &Path) -> Result<gix::Repository, PackageAcquisitionError> {
    gix::open::Options::isolated()
        .open(path)
        .map(|repository| repository.to_thread_local())
        .map_err(|error| {
            PackageAcquisitionError::invalid_git("open acquired Git repository", error)
        })
}

fn is_commit_id(revision: &str) -> bool {
    revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn commit_from_hex(
    repo: &gix::Repository,
    revision: &str,
) -> Result<gix::ObjectId, PackageAcquisitionError> {
    let id = gix::ObjectId::from_hex(revision.as_bytes())
        .map_err(|error| PackageAcquisitionError::invalid_git("decode Git commit", error))?;
    let object = repo
        .find_object(id)
        .map_err(|error| PackageAcquisitionError::invalid_git("find Git commit", error))?;
    let commit = object
        .peel_to_commit()
        .map_err(|error| PackageAcquisitionError::invalid_git("peel Git commit", error))?;
    Ok(commit.id)
}

fn required_reference(
    repo: &gix::Repository,
    name: &str,
    authored: &str,
) -> Result<gix::ObjectId, PackageAcquisitionError> {
    reference_commit(repo, name)?.ok_or_else(|| {
        PackageAcquisitionError::invalid_git(
            "resolve Git revision",
            format!("reference {authored:?} was not advertised by the repository"),
        )
    })
}

fn short_revision(
    repo: &gix::Repository,
    revision: &str,
) -> Result<gix::ObjectId, PackageAcquisitionError> {
    if revision.is_empty() || revision.contains(['~', '^', ':', ' ', '\t', '\n']) {
        return Err(PackageAcquisitionError::invalid_git(
            "resolve Git revision",
            "revision must be a commit, branch, or tag name",
        ));
    }
    let branch = reference_commit(repo, &format!("refs/remotes/origin/{revision}"))?;
    let tag = reference_commit(repo, &format!("refs/tags/{revision}"))?;
    match (branch, tag) {
        (Some(_), Some(_)) => Err(PackageAcquisitionError::invalid_git(
            "resolve Git revision",
            format!("short revision {revision:?} is ambiguous between a branch and tag"),
        )),
        (Some(id), None) | (None, Some(id)) => Ok(id),
        (None, None) => Err(PackageAcquisitionError::invalid_git(
            "resolve Git revision",
            format!("branch or tag {revision:?} was not advertised by the repository"),
        )),
    }
}

fn reference_commit(
    repo: &gix::Repository,
    name: &str,
) -> Result<Option<gix::ObjectId>, PackageAcquisitionError> {
    let Some(mut reference) = repo
        .try_find_reference(name)
        .map_err(|error| PackageAcquisitionError::invalid_git("find Git reference", error))?
    else {
        return Ok(None);
    };
    reference
        .peel_to_commit()
        .map(|commit| Some(commit.id))
        .map_err(|error| PackageAcquisitionError::invalid_git("peel Git reference", error))
}

#[derive(Default)]
struct MaterializationBudget {
    paths: BTreeSet<PathBuf>,
    entries: u64,
    bytes: u64,
}

fn export_tree(
    tree: &gix::Tree<'_>,
    destination: &Path,
    parent: &Path,
    depth: usize,
    budget: &mut MaterializationBudget,
) -> Result<(), PackageAcquisitionError> {
    if depth >= MAX_PATH_COMPONENTS {
        return Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "Git tree path exceeds 64 components",
        ));
    }
    for entry in tree.iter() {
        let entry = entry
            .map_err(|error| PackageAcquisitionError::invalid_git("decode Git tree", error))?;
        budget.entries += 1;
        if budget.entries > MAX_ENTRIES {
            return Err(PackageAcquisitionError::invalid_git(
                "materialize Git package",
                "Git tree exceeds 100,000 entries",
            ));
        }
        let name = portable_name(entry.filename())?;
        let relative = parent.join(name);
        if !budget.paths.insert(relative.clone()) {
            return Err(PackageAcquisitionError::invalid_git(
                "materialize Git package",
                format!("duplicate Git tree path {:?}", relative.display()),
            ));
        }
        let output = destination.join(&relative);
        match entry.kind() {
            EntryKind::Tree => {
                fs::create_dir(&output).map_err(|error| {
                    PackageAcquisitionError::filesystem("create Git tree directory", &output, error)
                })?;
                let object = entry.object().map_err(|error| {
                    PackageAcquisitionError::invalid_git("read Git subtree", error)
                })?;
                let subtree = object.try_into_tree().map_err(|error| {
                    PackageAcquisitionError::invalid_git("decode Git subtree", error)
                })?;
                export_tree(&subtree, destination, &relative, depth + 1, budget)?;
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                let executable = entry.kind() == EntryKind::BlobExecutable;
                let object = entry.object().map_err(|error| {
                    PackageAcquisitionError::invalid_git("read Git blob", error)
                })?;
                let blob = object.try_into_blob().map_err(|error| {
                    PackageAcquisitionError::invalid_git("decode Git blob", error)
                })?;
                budget.bytes = budget
                    .bytes
                    .checked_add(blob.data.len() as u64)
                    .ok_or_else(|| {
                        PackageAcquisitionError::invalid_git(
                            "materialize Git package",
                            "Git tree size overflow",
                        )
                    })?;
                if budget.bytes > MAX_MATERIALIZED_BYTES {
                    return Err(PackageAcquisitionError::invalid_git(
                        "materialize Git package",
                        "Git tree regular-file data exceeds 1 GiB",
                    ));
                }
                if blob.data.starts_with(LFS_POINTER_PREFIX) {
                    return Err(PackageAcquisitionError::invalid_git(
                        "materialize Git package",
                        format!("Git LFS pointer at {:?} is unsupported", relative.display()),
                    ));
                }
                write_blob(&output, &blob.data, executable)?;
            }
            EntryKind::Link => {
                return Err(PackageAcquisitionError::invalid_git(
                    "materialize Git package",
                    format!("symbolic link at {:?} is unsupported", relative.display()),
                ));
            }
            EntryKind::Commit => {
                return Err(PackageAcquisitionError::invalid_git(
                    "materialize Git package",
                    format!("submodule at {:?} is unsupported", relative.display()),
                ));
            }
        }
    }
    Ok(())
}

fn portable_name(name: &gix::bstr::BStr) -> Result<&str, PackageAcquisitionError> {
    let name = name.to_str().map_err(|_| {
        PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "Git tree paths must be valid UTF-8",
        )
    })?;
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
        return Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            format!("non-portable Git tree component {name:?}"),
        ));
    }
    Ok(name)
}

fn require_empty_directory(destination: &Path) -> Result<(), PackageAcquisitionError> {
    let metadata = fs::symlink_metadata(destination).map_err(|error| {
        PackageAcquisitionError::filesystem("inspect Git destination", destination, error)
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "destination is not a physical directory",
        ));
    }
    let mut entries = fs::read_dir(destination).map_err(|error| {
        PackageAcquisitionError::filesystem("read Git destination", destination, error)
    })?;
    if entries.next().is_some() {
        return Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "destination is not empty",
        ));
    }
    Ok(())
}

fn write_blob(
    output: &Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), PackageAcquisitionError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            PackageAcquisitionError::filesystem("create Git package file", output, error)
        })?;
    file.write_all(bytes).map_err(|error| {
        PackageAcquisitionError::filesystem("write Git package file", output, error)
    })?;
    set_file_mode(&file, output, executable)
}

#[cfg(unix)]
fn set_file_mode(
    file: &fs::File,
    path: &Path,
    executable: bool,
) -> Result<(), PackageAcquisitionError> {
    use std::os::unix::fs::PermissionsExt;

    let mode = if executable { 0o755 } else { 0o644 };
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| PackageAcquisitionError::filesystem("set Git file mode", path, error))
}

#[cfg(not(unix))]
fn set_file_mode(
    _file: &fs::File,
    _path: &Path,
    _executable: bool,
) -> Result<(), PackageAcquisitionError> {
    Ok(())
}

fn require_manifest(destination: &Path) -> Result<(), PackageAcquisitionError> {
    let manifest = destination.join("nocter.nct");
    match fs::symlink_metadata(&manifest) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(PackageAcquisitionError::invalid_git(
            "materialize Git package",
            "Git tree root nocter.nct is not a regular file",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(PackageAcquisitionError::invalid_git(
                "materialize Git package",
                "Git tree root does not contain nocter.nct",
            ))
        }
        Err(error) => Err(PackageAcquisitionError::filesystem(
            "inspect Git package manifest",
            manifest,
            error,
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "nocter-git-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    #[ignore = "requires public HTTPS"]
    fn clones_resolves_and_materializes_without_external_git() {
        let workspace = TempDirectory::new();
        let repository = workspace.0.join("repository.git");
        clone_repository("https://github.com/octocat/Hello-World.git", &repository).unwrap();
        let lock = resolve_revision(&repository, "master").unwrap();
        assert_eq!(lock.kind(), ExactDependencyLockKind::Git);

        let destination = workspace.0.join("package");
        fs::create_dir(&destination).unwrap();
        let error = export_commit(&repository, &lock, &destination).unwrap_err();
        assert!(destination.join("README").is_file());
        assert!(error.to_string().contains("does not contain nocter.nct"));
    }
}
