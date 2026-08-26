use nocter_model::{BorrowCapability, CallableCapability, ParameterOrigin};
use nocter_source::SourceMap;
use nocter_syntax::{NodeKind, ParseGoal};

use super::test_support::{add_source, all_nodes, bind, first_node, module, package, parse_source};
use super::{BoundDeclarationPattern, BoundInterfaceApplication, BoundTypeKind};
use crate::test_support::module_use;
use crate::{ModuleIdentity, PackageIdentity, SurfaceDeclarationId};

#[test]
fn binds_qualified_generic_and_associated_type_shapes_before_normalization() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let dep_manifest_id = add_source(&mut sources, "/dep/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "use dep\ntype Projection<T> = &dep.Buffer<T>.Item?!\n",
    );
    let dep_id = add_source(&mut sources, "/dep/index.nct", "pub struct Buffer<T> {}\n");
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let dep_manifest = parse_source(&sources, dep_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
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
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("resolved:dep", "dep", "/dep/index.nct", &dep_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
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
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "type Callback<T> = &+func(left: &T, right: &T): &T from right | left\n",
    );
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
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
fn binds_instance_patterns_and_interface_applications_to_generic_identities() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "struct Pair<T> {}\n",
            "interface Show<T> {}\n",
            "instance Pair<T> { impl Show<T> }\n",
            "func inspect<T>(outer: &T): void ",
            "where T impl Show<T> { return }\n",
        ),
    );
    let std_root_id = add_source(
        &mut sources,
        "/std/index.nct",
        crate::test_support::TEST_BUILTIN_SOURCE,
    );
    let prelude_id = add_source(&mut sources, "/std/prelude/index.nct", "");
    let app_manifest = parse_source(&sources, app_manifest_id, ParseGoal::SourceFile);
    let std_manifest = parse_source(&sources, std_manifest_id, ParseGoal::SourceFile);
    let app = parse_source(&sources, app_id, ParseGoal::SourceFile);
    let std_root = parse_source(&sources, std_root_id, ParseGoal::SourceFile);
    let prelude = parse_source(&sources, prelude_id, ParseGoal::SourceFile);
    let prelude_identity = ModuleIdentity::new(PackageIdentity::new("builtin:std"), ["prelude"]);
    let bound = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
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
        .declaration_patterns(SurfaceDeclarationId::from_index(
            nocter_model::BuiltinType::COUNT + 2,
        ))
        .unwrap();
    assert!(matches!(
        instance,
        [BoundDeclarationPattern::Nominal { arguments, .. }] if arguments.len() == 1
    ));
    let applications = all_nodes(&app, NodeKind::InterfaceApplication);
    assert_eq!(applications.len(), 2);
    assert!(applications.iter().all(|application| matches!(
        bound.interface_application_for(*application),
        Some(BoundInterfaceApplication { arguments, .. }) if arguments.len() == 1
    )));
}
