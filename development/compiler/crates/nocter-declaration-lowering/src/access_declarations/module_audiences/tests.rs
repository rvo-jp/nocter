use super::*;
use nocter_model::PackageIdentity;
use nocter_source::{SourceMap, SourceName};
use nocter_syntax::{ParseGoal, parse};

fn module(package: &str, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied())
}
fn authored(
    modules: &[ModuleIdentity],
    declaring: &ModuleIdentity,
    path: &str,
) -> Result<ModuleAudience, Failure> {
    let text = format!("#visibility: {path:?}\nfunc api(): void");
    let mut sources = SourceMap::new();
    let id = sources
        .add_bytes(SourceName::new("index.nct"), text.as_bytes())
        .unwrap();
    let tree = parse(sources.get(id).unwrap(), ParseGoal::V2CoreUnit);
    assert!(!tree.has_errors(), "{:?}", tree.diagnostics());
    let syntax = BoundSyntax::new(sources.get(id).unwrap(), &tree).unwrap();
    let directive = tree
        .nodes()
        .find(|(_, node)| node.kind() == NodeKind::Visibility)
        .unwrap()
        .0;
    resolve(modules, declaring, syntax, Some(directive))
}
#[test]
fn relative_paths_resolve_once_from_the_declaring_module_and_cannot_cross_the_package_root() {
    let modules = [
        module("app", &[]),
        module("app", &["codec"]),
        module("app", &["codec", "format"]),
        module("other", &[]),
    ];
    assert_eq!(
        authored(&modules, &modules[2], "/").unwrap(),
        ModuleAudience::Package(modules[0].package().clone())
    );
    assert_eq!(
        authored(&modules, &modules[2], "./").unwrap(),
        ModuleAudience::Descendants(modules[2].clone())
    );
    assert_eq!(
        authored(&modules, &modules[2], "../").unwrap(),
        ModuleAudience::Descendants(modules[1].clone())
    );
    assert_eq!(
        authored(&modules, &modules[2], "../../").unwrap(),
        ModuleAudience::Package(modules[0].package().clone())
    );
    for path in [
        "", ".", "..", "package", "codec", "/codec", "./codec", "../codec", "./../", "/../",
    ] {
        assert!(
            matches!(authored(&modules, &modules[2], path), Err(Failure::Visibility(error)) if error.rule() == VisibilityRule::InvalidPath),
            "{path}"
        );
    }
    assert!(
        matches!(authored(&modules, &modules[2], "../../../"), Err(Failure::Visibility(error)) if error.rule() == VisibilityRule::AbovePackageRoot)
    );
    let source_folder = module("app", &["codec", "details", "format"]);
    assert!(
        matches!(authored(&modules, &source_folder, "../"), Err(Failure::Visibility(error)) if error.rule() == VisibilityRule::InvalidPath)
    );
}
