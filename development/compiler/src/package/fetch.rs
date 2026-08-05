use super::{DependencyLock, DependencySource, PackageId};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct FetchResult {
    pub(super) root: PathBuf,
    pub(super) resolution: DependencyLock,
}

pub(super) fn resolve_git_revision(url: &str, revision: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["ls-remote", url, revision])
        .output()
        .map_err(|error| format!("failed to start Git: {error}"))?;
    if !output.status.success() {
        return Err(tool_error("git ls-remote", &output.stderr));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let commit = stdout
        .lines()
        .find_map(|line| line.split_whitespace().next())
        .filter(|commit| is_git_object_id(commit))
        .ok_or_else(|| format!("Git revision `{revision}` does not resolve in `{url}`"))?;
    Ok(commit.to_ascii_lowercase())
}

pub(super) fn fetch(
    source: &DependencySource,
    resolution: &DependencyLock,
    id: &PackageId,
    store_root: &Path,
) -> Result<FetchResult, String> {
    fs::create_dir_all(store_root)
        .map_err(|error| format!("failed to create package store: {error}"))?;
    let temporary = store_root.join(format!(".fetch-{}", unique_suffix()));
    if temporary.exists() {
        return Err(format!(
            "temporary package path unexpectedly exists: `{}`",
            temporary.display()
        ));
    }
    let result = match (source, resolution) {
        (DependencySource::Git { url, .. }, DependencyLock::GitCommit(commit)) => {
            fetch_git(url, commit, &temporary)
        }
        (DependencySource::Archive { url }, DependencyLock::ArchiveSha256(digest)) => {
            fetch_archive(url, digest, &temporary)
        }
        _ => Err("dependency source and lock kinds do not match".to_string()),
    };
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&temporary);
        return Err(error);
    }
    validate_package_tree(&temporary)?;
    let destination = store_root.join(id.as_str());
    if destination.exists() {
        let _ = fs::remove_dir_all(&temporary);
    } else {
        fs::rename(&temporary, &destination)
            .map_err(|error| format!("failed to install fetched package: {error}"))?;
    }
    Ok(FetchResult {
        root: destination,
        resolution: resolution.clone(),
    })
}

fn fetch_git(url: &str, commit: &str, destination: &Path) -> Result<(), String> {
    run_git(["init", "--quiet"], destination, true)?;
    run_git(["remote", "add", "origin", url], destination, false)?;
    run_git(
        ["fetch", "--quiet", "--depth", "1", "origin", commit],
        destination,
        false,
    )?;
    run_git(
        ["checkout", "--quiet", "--detach", "FETCH_HEAD"],
        destination,
        false,
    )?;
    let git = destination.join(".git");
    fs::remove_dir_all(&git)
        .map_err(|error| format!("failed to remove fetched Git metadata: {error}"))?;
    Ok(())
}

fn run_git<const N: usize>(
    arguments: [&str; N],
    directory: &Path,
    create: bool,
) -> Result<(), String> {
    if create {
        fs::create_dir_all(directory)
            .map_err(|error| format!("failed to create Git package directory: {error}"))?;
    }
    let output = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(|error| format!("failed to start Git: {error}"))?;
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| tool_error("git", &output.stderr))
}

fn fetch_archive(url: &str, expected: &str, destination: &Path) -> Result<(), String> {
    let parent = destination
        .parent()
        .ok_or_else(|| "archive destination has no parent".to_string())?;
    let archive = parent.join(format!(".archive-{}.tar.gz", unique_suffix()));
    let output = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(&archive)
        .arg(url)
        .output()
        .map_err(|error| format!("failed to start curl: {error}"))?;
    if !output.status.success() {
        return Err(tool_error("curl", &output.stderr));
    }
    let digest = sha256_file(&archive)?;
    if digest != expected {
        let _ = fs::remove_file(&archive);
        return Err(format!(
            "archive digest mismatch: expected sha256:{expected}, received sha256:{digest}"
        ));
    }
    let listing = Command::new("tar")
        .args(["-tzf"])
        .arg(&archive)
        .output()
        .map_err(|error| format!("failed to start tar: {error}"))?;
    if !listing.status.success() {
        let _ = fs::remove_file(&archive);
        return Err(tool_error("tar", &listing.stderr));
    }
    for entry in String::from_utf8_lossy(&listing.stdout).lines() {
        let path = Path::new(entry);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            let _ = fs::remove_file(&archive);
            return Err(format!("archive contains unsafe path `{entry}`"));
        }
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("failed to create archive package directory: {error}"))?;
    let extract = Command::new("tar")
        .args(["-xzf"])
        .arg(&archive)
        .arg("-C")
        .arg(destination)
        .output()
        .map_err(|error| format!("failed to start tar: {error}"))?;
    let _ = fs::remove_file(&archive);
    extract
        .status
        .success()
        .then_some(())
        .ok_or_else(|| tool_error("tar", &extract.stderr))
}

pub(super) fn archive_digest(url: &str, directory: &Path) -> Result<String, String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("failed to create package store: {error}"))?;
    let archive = directory.join(format!(".resolve-{}.tar.gz", unique_suffix()));
    let output = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--output",
        ])
        .arg(&archive)
        .arg(url)
        .output()
        .map_err(|error| format!("failed to start curl: {error}"))?;
    if !output.status.success() {
        return Err(tool_error("curl", &output.stderr));
    }
    let digest = sha256_file(&archive);
    let _ = fs::remove_file(archive);
    digest
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("failed to open downloaded archive: {error}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read downloaded archive: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn validate_package_tree(root: &Path) -> Result<(), String> {
    if !root.join("nocter.nct").is_file() {
        return Err("fetched package does not contain a root `nocter.nct`".to_string());
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize fetched package: {error}"))?;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("failed to inspect fetched package: {error}"))?
        {
            let path = entry
                .map_err(|error| format!("failed to inspect fetched package: {error}"))?
                .path();
            let canonical = path.canonicalize().map_err(|error| {
                format!(
                    "failed to canonicalize fetched entry `{}`: {error}",
                    path.display()
                )
            })?;
            if !canonical.starts_with(&canonical_root) {
                return Err(format!(
                    "fetched package entry `{}` escapes the package root",
                    path.display()
                ));
            }
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    Ok(())
}

fn is_git_object_id(value: &str) -> bool {
    (value.len() == 40 || value.len() == 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn unique_suffix() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn tool_error(tool: &str, stderr: &[u8]) -> String {
    let detail = String::from_utf8_lossy(stderr).trim().to_string();
    if detail.is_empty() {
        format!("{tool} failed")
    } else {
        format!("{tool} failed: {detail}")
    }
}
