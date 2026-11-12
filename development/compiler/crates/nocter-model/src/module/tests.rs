use super::*;

fn module(package: &str, path: &[&str]) -> ModuleIdentity {
    ModuleIdentity::new(PackageIdentity::new(package), path.iter().copied())
}

#[test]
fn audience_inclusion_and_owner_intersection_respect_exact_package_identity() {
    let root = module("app", &[]);
    let codec = module("app", &["codec"]);
    let child = module("app", &["codec", "format"]);
    let unrelated = module("app", &["other"]);
    let foreign = module("other", &["codec", "format"]);
    let package = ModuleAudience::Package(root.package().clone());
    let branch = ModuleAudience::Descendants(codec.clone());
    let narrow = ModuleAudience::Descendants(child.clone());
    assert!(branch.contains(&child));
    assert!(!branch.contains(&unrelated));
    assert!(!branch.contains(&foreign));
    assert!(narrow.is_within(&branch));
    assert!(branch.is_within(&package));
    assert!(!package.is_within(&branch));
    assert!(!ModuleAudience::Public.is_within(&package));
    assert_eq!(
        ModuleAudience::Public.intersection(&branch),
        Some(branch.clone())
    );
    assert_eq!(package.clone().intersection(&narrow), Some(narrow));
    assert_eq!(
        branch.intersection(&ModuleAudience::Package(foreign.package().clone())),
        None
    );
    assert!(package.is_within(&ModuleAudience::Descendants(root)));
}
