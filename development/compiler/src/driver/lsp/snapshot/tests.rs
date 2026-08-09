use super::*;
use crate::driver::lsp::documents::{WorkspaceRoot, open_document};
use crate::driver::lsp::protocol::file_uri_for_path;
use crate::driver::lsp::tests::NocterHomeEnv;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn stable_inputs_reuse_one_immutable_generation() {
    let project = TempProject::new("stable-generation");
    let source = project.write("index.nct", "pub func value(): i32 { 1 }\n");
    let uri = file_uri_for_path(&source);
    let documents = HashMap::from([(
        uri.clone(),
        open_document(uri, Some(1), fs::read_to_string(&source).unwrap()),
    )]);
    let store = SnapshotStore::default();

    let first = store.current(&documents, &[]);
    let second = store.current(&documents, &[]);

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.generation(), second.generation());
}

#[test]
fn invalidation_follows_reverse_imports_and_reuses_unrelated_analyses() {
    let project = TempProject::new("reverse-import-invalidation");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write(
        "index.nct",
        "use ./config.value\n\npub func read(): i32 { value() }\n",
    );
    let config = project.write("config/index.nct", "pub func value(): i32 { 1 }\n");
    let other = project.write("other.nct", "pub func other(): i32 { 2 }\n");
    let mut documents = documents_for([&app, &config, &other]);
    let store = SnapshotStore::default();
    let first = store.current(&documents, &[]);

    let config_uri = file_uri_for_path(&config);
    let config_document = documents.get_mut(&config_uri).unwrap();
    config_document.version = Some(2);
    config_document.text = "pub func value(): i32 { 3 }\n".to_string();
    let second = store.rebuild(
        &documents,
        &[],
        SnapshotChange::path(Some(config.as_path())),
    );

    let app_uri = file_uri_for_path(&app);
    let other_uri = file_uri_for_path(&other);
    assert!(!Arc::ptr_eq(
        &first.document_snapshot(&app_uri).unwrap().analysis,
        &second.document_snapshot(&app_uri).unwrap().analysis,
    ));
    assert!(!Arc::ptr_eq(
        &first.document_snapshot(&config_uri).unwrap().analysis,
        &second.document_snapshot(&config_uri).unwrap().analysis,
    ));
    assert!(Arc::ptr_eq(
        &first.document_snapshot(&other_uri).unwrap().analysis,
        &second.document_snapshot(&other_uri).unwrap().analysis,
    ));
}

#[test]
fn unsaved_package_manifest_overlay_drives_the_shared_graph() {
    let project = TempProject::new("manifest-overlay");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let package_file = project.write("nocter.nct", "#name: \"app\"\n");
    let app = project.write(
        "index.nct",
        "use math.answer\n\npub func value(): i32 { answer() }\n",
    );
    project.write("math/nocter.nct", "#name: \"math\"\n");
    project.write("math/index.nct", "pub func answer(): i32 { 42 }\n");
    let package_uri = file_uri_for_path(&package_file);
    let app_uri = file_uri_for_path(&app);
    let mut documents = documents_for([&package_file, &app]);
    let workspace_roots = vec![WorkspaceRoot {
        uri: file_uri_for_path(&project.root),
        path: Some(project.root.clone()),
    }];
    let store = SnapshotStore::default();
    let before = store.current(&documents, &workspace_roots);
    let package_document = documents.get_mut(&package_uri).unwrap();
    package_document.version = Some(2);
    package_document.text =
        "#name: \"app\"\n#dependencies: { math: { path: \"./math\" } }\n".to_string();

    let snapshot = store.rebuild(
        &documents,
        &workspace_roots,
        SnapshotChange::path(Some(package_file.as_path())),
    );
    let graph = snapshot
        .package_graph(&app_uri)
        .expect("unsaved package directives should produce a graph");
    let aliases = graph
        .dependency_names(graph.root_package().id())
        .collect::<Vec<_>>();

    assert_eq!(aliases, vec!["math"]);
    assert!(!Arc::ptr_eq(
        &before.document_snapshot(&app_uri).unwrap().analysis,
        &snapshot.document_snapshot(&app_uri).unwrap().analysis,
    ));
    assert_eq!(
        fs::read_to_string(&package_file).unwrap(),
        "#name: \"app\"\n"
    );
    assert!(
        snapshot
            .analysis(&app_uri)
            .unwrap()
            .diagnostics()
            .is_empty(),
        "{:?}",
        snapshot.analysis(&app_uri).unwrap().diagnostics()
    );
}

#[test]
fn malformed_manifest_invalidates_the_graph_and_recovery_restores_it() {
    let project = TempProject::new("manifest-recovery");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let valid_manifest = "#name: \"app\"\n#dependencies: { math: { path: \"./math\" } }\n";
    let package_file = project.write("nocter.nct", valid_manifest);
    let app = project.write(
        "index.nct",
        "use math.answer\n\npub func value(): i32 { answer() }\n",
    );
    project.write("math/nocter.nct", "#name: \"math\"\n");
    project.write("math/index.nct", "pub func answer(): i32 { 42 }\n");
    let package_uri = file_uri_for_path(&package_file);
    let app_uri = file_uri_for_path(&app);
    let mut documents = documents_for([&package_file, &app]);
    let workspace_roots = vec![WorkspaceRoot {
        uri: file_uri_for_path(&project.root),
        path: Some(project.root.clone()),
    }];
    let store = SnapshotStore::default();
    let valid = store.current(&documents, &workspace_roots);
    assert!(valid.package_graph(&app_uri).is_some());
    assert!(valid.analysis(&app_uri).unwrap().semantic().is_some());

    let package_document = documents.get_mut(&package_uri).unwrap();
    package_document.version = Some(2);
    package_document.text = "#dependencies: { math: }\n".to_string();
    let malformed = store.rebuild(
        &documents,
        &workspace_roots,
        SnapshotChange::path(Some(package_file.as_path())),
    );

    assert!(malformed.package_graph(&app_uri).is_none());
    assert!(malformed.analysis(&app_uri).unwrap().semantic().is_none());
    assert!(!malformed.diagnostics_for_uri(&package_uri).is_empty());
    assert!(!Arc::ptr_eq(
        &valid.document_snapshot(&app_uri).unwrap().analysis,
        &malformed.document_snapshot(&app_uri).unwrap().analysis,
    ));

    let package_document = documents.get_mut(&package_uri).unwrap();
    package_document.version = Some(3);
    package_document.text = valid_manifest.to_string();
    let repaired = store.rebuild(
        &documents,
        &workspace_roots,
        SnapshotChange::path(Some(package_file.as_path())),
    );

    assert!(repaired.package_graph(&app_uri).is_some());
    assert!(repaired.analysis(&app_uri).unwrap().semantic().is_some());
    assert!(repaired.diagnostics_for_uri(&package_uri).is_empty());
    assert!(!Arc::ptr_eq(
        &malformed.document_snapshot(&app_uri).unwrap().analysis,
        &repaired.document_snapshot(&app_uri).unwrap().analysis,
    ));
}

#[test]
fn repairing_a_failed_dependency_manifest_reloads_the_package_graph() {
    let project = TempProject::new("dependency-manifest-recovery");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    project.write(
        "nocter.nct",
        "#name: \"app\"\n#dependencies: { math: { path: \"./math\" } }\n",
    );
    let app = project.write(
        "index.nct",
        "use math.answer\n\npub func value(): i32 { answer() }\n",
    );
    let dependency_manifest = project.write("math/nocter.nct", "#name: \"math\"\n");
    project.write("math/index.nct", "pub func answer(): i32 { 42 }\n");
    let app_uri = file_uri_for_path(&app);
    let documents = documents_for([&app]);
    let workspace_roots = vec![WorkspaceRoot {
        uri: file_uri_for_path(&project.root),
        path: Some(project.root.clone()),
    }];
    let store = SnapshotStore::default();
    let valid = store.current(&documents, &workspace_roots);
    assert!(valid.package_graph(&app_uri).is_some());
    assert!(valid.analysis(&app_uri).unwrap().semantic().is_some());

    crate::test_files::write(&dependency_manifest, "#name: \"math\n").unwrap();
    let malformed = store.rebuild(
        &documents,
        &workspace_roots,
        SnapshotChange::path(Some(dependency_manifest.as_path())),
    );
    assert!(malformed.package_graph(&app_uri).is_none());
    assert!(malformed.analysis(&app_uri).unwrap().semantic().is_none());

    crate::test_files::write(&dependency_manifest, "#name: \"math\"\n").unwrap();
    let repaired = store.rebuild(
        &documents,
        &workspace_roots,
        SnapshotChange::path(Some(dependency_manifest.as_path())),
    );

    assert!(repaired.package_graph(&app_uri).is_some());
    assert!(repaired.analysis(&app_uri).unwrap().semantic().is_some());
    assert!(!Arc::ptr_eq(
        &malformed.document_snapshot(&app_uri).unwrap().analysis,
        &repaired.document_snapshot(&app_uri).unwrap().analysis,
    ));
}

#[test]
fn watched_disk_dependency_change_invalidates_its_importers() {
    let project = TempProject::new("watched-dependency");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write(
        "index.nct",
        "use ./config.value\n\npub func read(): i32 { value() }\n",
    );
    let config = project.write("config/index.nct", "pub func value(): i32 { 1 }\n");
    let documents = documents_for([&app]);
    let store = SnapshotStore::default();
    let first = store.current(&documents, &[]);

    crate::test_files::write(&config, "pub func value(): i32 { 2 }\n").unwrap();
    let second = store.rebuild(
        &documents,
        &[],
        SnapshotChange::path(Some(config.as_path())),
    );
    let app_uri = file_uri_for_path(&app);

    assert!(!Arc::ptr_eq(
        &first.document_snapshot(&app_uri).unwrap().analysis,
        &second.document_snapshot(&app_uri).unwrap().analysis,
    ));
}

#[test]
fn repairing_a_failed_disk_dependency_rebuilds_its_importer() {
    let project = TempProject::new("failed-watched-dependency");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write(
        "index.nct",
        "use ./config.value\n\npub func read(): i32 { value() }\n",
    );
    let config = project.write("config/index.nct", "pub func value(: i32 {\n");
    let documents = documents_for([&app]);
    let store = SnapshotStore::default();
    let failed = store.current(&documents, &[]);
    let app_uri = file_uri_for_path(&app);
    assert!(failed.analysis(&app_uri).unwrap().semantic().is_none());

    crate::test_files::write(&config, "pub func value(): i32 { 2 }\n").unwrap();
    let repaired = store.rebuild(
        &documents,
        &[],
        SnapshotChange::path(Some(config.as_path())),
    );

    assert!(!Arc::ptr_eq(
        &failed.document_snapshot(&app_uri).unwrap().analysis,
        &repaired.document_snapshot(&app_uri).unwrap().analysis,
    ));
    assert!(repaired.analysis(&app_uri).unwrap().semantic().is_some());
    assert!(
        repaired
            .analysis(&app_uri)
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[test]
fn creating_a_previously_missing_dependency_rebuilds_its_importer() {
    let project = TempProject::new("created-dependency");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write(
        "index.nct",
        "use ./config.value\n\npub func read(): i32 { value() }\n",
    );
    let config = project.root.join("config/index.nct");
    let documents = documents_for([&app]);
    let store = SnapshotStore::default();
    let missing = store.current(&documents, &[]);
    let app_uri = file_uri_for_path(&app);
    assert!(missing.analysis(&app_uri).unwrap().semantic().is_none());

    crate::test_files::write(&config, "pub func value(): i32 { 2 }\n").unwrap();
    let repaired = store.rebuild(
        &documents,
        &[],
        SnapshotChange::path(Some(config.as_path())),
    );

    assert!(!Arc::ptr_eq(
        &missing.document_snapshot(&app_uri).unwrap().analysis,
        &repaired.document_snapshot(&app_uri).unwrap().analysis,
    ));
    assert!(repaired.analysis(&app_uri).unwrap().semantic().is_some());
    assert!(
        repaired
            .analysis(&app_uri)
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn deleting_a_symlinked_dependency_rebuilds_its_importer() {
    use std::os::unix::fs::symlink;

    let project = TempProject::new("deleted-symlink-dependency");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let app = project.write(
        "index.nct",
        "use ./config.value\n\npub func read(): i32 { value() }\n",
    );
    let target = project.write("shared/index.nct", "pub func value(): i32 { 1 }\n");
    let link = project.root.join("config/index.nct");
    fs::create_dir_all(link.parent().unwrap()).unwrap();
    symlink(&target, &link).unwrap();
    let documents = documents_for([&app]);
    let store = SnapshotStore::default();
    let available = store.current(&documents, &[]);
    let app_uri = file_uri_for_path(&app);
    assert!(available.analysis(&app_uri).unwrap().semantic().is_some());

    fs::remove_file(&link).unwrap();
    let missing = store.rebuild(&documents, &[], SnapshotChange::path(Some(link.as_path())));

    assert!(!Arc::ptr_eq(
        &available.document_snapshot(&app_uri).unwrap().analysis,
        &missing.document_snapshot(&app_uri).unwrap().analysis,
    ));
    assert!(missing.analysis(&app_uri).unwrap().semantic().is_none());
}

#[test]
fn nested_packages_receive_distinct_graph_snapshots() {
    let project = TempProject::new("nested-packages");
    let root_package = project.write("nocter.nct", "#name: \"root\"\n");
    let root_source = project.write("index.nct", "pub func root(): i32 { 1 }\n");
    let nested_package = project.write("nested/nocter.nct", "#name: \"nested\"\n");
    let nested_source = project.write("nested/index.nct", "pub func nested(): i32 { 2 }\n");
    let documents = documents_for([&root_package, &root_source, &nested_package, &nested_source]);
    let workspace_roots = vec![WorkspaceRoot {
        uri: file_uri_for_path(&project.root),
        path: Some(project.root.clone()),
    }];
    let snapshot = SnapshotStore::default().current(&documents, &workspace_roots);

    let root_graph = snapshot
        .package_graph(&file_uri_for_path(&root_source))
        .unwrap();
    let nested_graph = snapshot
        .package_graph(&file_uri_for_path(&nested_source))
        .unwrap();
    assert_eq!(root_graph.root_package().display_name(), "root");
    assert_eq!(nested_graph.root_package().display_name(), "nested");
    assert_ne!(
        root_graph.root_package().id(),
        nested_graph.root_package().id()
    );
}

#[test]
fn diagnostics_for_independent_documents_survive_one_snapshot() {
    let project = TempProject::new("independent-diagnostics");
    let first = project.write("first.nct", "func first(: i32 {\n");
    let second = project.write("second.nct", "func second(: i32 {\n");
    let documents = documents_for([&first, &second]);
    let snapshot = SnapshotStore::default().current(&documents, &[]);

    assert!(
        !snapshot
            .diagnostics_for_uri(&file_uri_for_path(&first))
            .is_empty()
    );
    assert!(
        !snapshot
            .diagnostics_for_uri(&file_uri_for_path(&second))
            .is_empty()
    );
}

#[test]
fn large_workspace_rebuilds_exactly_the_affected_partition() {
    let project = TempProject::new("large-invalidation-partition");
    let home = project.write_nocter_home();
    let _home = NocterHomeEnv::set(&home);
    let shared = project.write("shared/index.nct", "pub func value(): i32 { 1 }\n");
    let mut paths = vec![shared.clone()];
    let mut importers = Vec::new();
    let mut independent = Vec::new();
    for index in 0..24 {
        let path = project.write(
            &format!("consumer_{index}.nct"),
            &format!("use ./shared.value\n\npub func read_{index}(): i32 {{ value() }}\n"),
        );
        paths.push(path.clone());
        importers.push(path);
    }
    for index in 0..24 {
        let path = project.write(
            &format!("independent_{index}.nct"),
            &format!("pub func value_{index}(): i32 {{ {index} }}\n"),
        );
        paths.push(path.clone());
        independent.push(path);
    }
    let mut documents = documents_for(paths.iter());
    let store = SnapshotStore::default();
    let first = store.current(&documents, &[]);

    let shared_uri = file_uri_for_path(&shared);
    let shared_document = documents.get_mut(&shared_uri).unwrap();
    shared_document.version = Some(2);
    shared_document.text = "pub func value(): i32 { 2 }\n".to_string();
    let second = store.rebuild(
        &documents,
        &[],
        SnapshotChange::path(Some(shared.as_path())),
    );

    assert!(!Arc::ptr_eq(
        &first.document_snapshot(&shared_uri).unwrap().analysis,
        &second.document_snapshot(&shared_uri).unwrap().analysis,
    ));
    for path in importers {
        let uri = file_uri_for_path(&path);
        assert!(!Arc::ptr_eq(
            &first.document_snapshot(&uri).unwrap().analysis,
            &second.document_snapshot(&uri).unwrap().analysis,
        ));
    }
    for path in independent {
        let uri = file_uri_for_path(&path);
        assert!(Arc::ptr_eq(
            &first.document_snapshot(&uri).unwrap().analysis,
            &second.document_snapshot(&uri).unwrap().analysis,
        ));
    }
}

fn documents_for<'a>(
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> HashMap<String, crate::driver::lsp::documents::OpenDocument> {
    paths
        .into_iter()
        .map(|path| {
            let uri = file_uri_for_path(path);
            let document = open_document(uri.clone(), Some(1), fs::read_to_string(path).unwrap());
            (uri, document)
        })
        .collect()
}

struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "nocter-lsp-snapshot-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        crate::test_files::write(&path, text).unwrap();
        path.canonicalize().unwrap()
    }

    fn write_nocter_home(&self) -> PathBuf {
        let home = self.root.join(".nocter");
        fs::create_dir_all(home.join("std/prelude")).unwrap();
        fs::create_dir_all(home.join("std/str")).unwrap();
        fs::create_dir_all(home.join("std/slice")).unwrap();
        crate::test_files::write(home.join("std/prelude/index.nct"), "").unwrap();
        crate::test_files::write(
            home.join("std/str/index.nct"),
            "pub(/) primitive str_len_raw(value: &str): usize\nimpl str { pub method &self.len(): usize { return str_len_raw(self) } pub method &self.is_empty(): bool { return str_len_raw(self) == 0 } }\n",
        )
        .unwrap();
        crate::test_files::write(
            home.join("std/slice/index.nct"),
            "pub(/) primitive slice_len_raw<T>(value: &[T]): usize\nimpl<T> [T] { pub method &self.len(): usize { return slice_len_raw(self) } pub method &self.is_empty(): bool { return slice_len_raw(self) == 0 } }\n",
        )
        .unwrap();
        home
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
