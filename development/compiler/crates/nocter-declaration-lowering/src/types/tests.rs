use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin};
use nocter_source::SourceMap;
use nocter_syntax::{NodeKind, ParseGoal};

use super::test_support::{add_source, all_nodes, bind, first_node, module, package, parse_source};
use super::{BoundCapability, BoundDeclarationPattern, BoundTypeKind};
use crate::test_support::module_use;
use crate::{ModuleIdentity, PackageIdentity, SurfaceDeclarationId};

#[test]
fn binds_qualified_generic_and_associated_type_shapes_before_normalization() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep\ntype Projection<T> = &dep.Buffer<T>.Item?!\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Buffer<T> {}\n");
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let dep = parse_source(&sources, dep_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let dep_identity =
        ModuleIdentity::new(PackageIdentity::new("resolved:dep"), Vec::<&str>::new());
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/nocter.nct", &dep_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app),
            module("resolved:dep", &[], "/dep/index.nct", &dep),
            module("builtin:std", &[], "/std/index.nct", &std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude,
            ),
        ],
        vec![module_use(&app, 0, dep_identity)],
        &prelude_identity,
    )
    .unwrap();
    let root = bound.type_for(first_node(&app, NodeKind::Type)).unwrap();
    let BoundTypeKind::Fallible(optional) = bound.kind(root).unwrap() else {
        panic!("expected outer fallible type");
    };
    let BoundTypeKind::Optional(borrowed) = bound.kind(*optional).unwrap() else {
        panic!("expected optional success payload");
    };
    let BoundTypeKind::Borrow {
        capability: BorrowCapability::Readonly,
        referent,
    } = bound.kind(*borrowed).unwrap()
    else {
        panic!("expected readonly borrow");
    };
    let BoundTypeKind::AssociatedSelection { base, .. } = bound.kind(*referent).unwrap() else {
        panic!("expected unresolved associated selection");
    };
    let BoundTypeKind::Nominal { arguments, .. } = bound.kind(*base).unwrap() else {
        panic!("expected qualified nominal base");
    };
    assert!(matches!(
        bound.kind(arguments[0]),
        Some(BoundTypeKind::GenericParameter(_))
    ));
}

#[test]
fn normalizes_explicit_callable_origins_to_parameter_positions() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "type Callback<T> = &+func(left: &T, right: &T): &T from right | left\n",
    );
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app),
            module("builtin:std", &[], "/std/index.nct", &std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude,
            ),
        ],
        vec![],
        &prelude_identity,
    )
    .unwrap();
    let root = bound.type_for(first_node(&app, NodeKind::Type)).unwrap();
    let BoundTypeKind::Callable(callable) = bound.kind(root).unwrap() else {
        panic!("expected callable type");
    };

    assert_eq!(callable.capability(), CallableCapability::ReadWrite);
    assert_eq!(callable.parameters().len(), 2);
    assert_eq!(
        callable.explicit_origins(),
        Some([ParameterOrigin::new(0), ParameterOrigin::new(1)].as_slice())
    );
}

#[test]
fn binds_nominal_and_interface_patterns_to_their_generic_identities() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/nocter.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/nocter.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "struct Pair<T> {}\n",
            "interface Show<T> {}\n",
            "instance Pair<T> {}\n",
            "conform Show<T> for Pair<T> {}\n",
            "func inspect<T>(outer: &T): void ",
            "where T: Show<T> + &func(value: &T): &T from value { return }\n",
        ),
    );
    let std_root_id = add_source(&mut sources, "/std/index.nct", "");
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::PackageFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::PackageFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/nocter.nct", &app_manifest),
            package("builtin:std", "std", "/std/nocter.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/index.nct", &app),
            module("builtin:std", &[], "/std/index.nct", &std_root),
            module(
                "builtin:std",
                &["prelude"],
                "/std/prelude/index.nct",
                &prelude,
            ),
        ],
        vec![],
        &prelude_identity,
    )
    .unwrap();
    let instance = bound
        .declaration_patterns(SurfaceDeclarationId::from_index(2))
        .unwrap();
    let conform = bound
        .declaration_patterns(SurfaceDeclarationId::from_index(3))
        .unwrap();

    assert!(matches!(
        instance,
        [BoundDeclarationPattern::Nominal { arguments, .. }] if arguments.len() == 1
    ));
    assert!(matches!(
        conform,
        [BoundDeclarationPattern::Interface { arguments: interface, .. },
         BoundDeclarationPattern::Nominal { arguments: target, .. }]
            if interface == target && interface.len() == 1
    ));
    let capabilities = all_nodes(&app, NodeKind::Capability);
    assert!(matches!(
        bound.capability_for(capabilities[0]),
        Some(BoundCapability::Interface { arguments, .. }) if arguments.len() == 1
    ));
    let Some(BoundCapability::Callable(callable)) = bound.capability_for(capabilities[1]) else {
        panic!("expected structural callable capability");
    };
    assert!(matches!(
        bound.kind(*callable),
        Some(BoundTypeKind::Callable(contract))
            if contract.explicit_origins() == Some([ParameterOrigin::new(0)].as_slice())
    ));
}
