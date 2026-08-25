use nocter_declarations::RequirementSubject;
use nocter_source::SourceMap;
use nocter_syntax::ParseGoal;

use super::test_support::{add_source, bind, module, package, parse_source};
use super::{
    BoundCapability, BoundRequirementKind, BoundTypeKind, PreparedTypeBindings, TypeBindingError,
};
use crate::{ModuleIdentity, PackageIdentity, SurfaceDeclarationId};

#[test]
fn binds_every_requirement_family_and_associated_type_bounds() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        concat!(
            "interface Show<T> {}\n",
            "interface Source {\n    pub type Item: Show<i32>\n}\n",
            "func inspect<T, U, C, I>(value: T): void where ",
            "T: Show<U> + &func(value: &T): &T from value, ",
            "copy U, T.Item = U, (&T == &T): bool, (&T < &T): bool, ",
            "(&C[usize]): &U, &T as &str, (...&C): I { return }\n",
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

    assert_requirement_families(&bound);
}

fn assert_requirement_families(bound: &PreparedTypeBindings<'_>) {
    let associated = bound
        .declaration_requirements(SurfaceDeclarationId::from_index(
            nocter_model::BuiltinType::COUNT + 2,
        ))
        .unwrap();
    assert!(matches!(
        associated,
        [BoundRequirementKind::Capability {
            subject: RequirementSubject::AssociatedType(_),
            capability: BoundCapability::Interface { arguments, .. },
        }] if arguments.len() == 1
    ));
    let requirements = bound
        .declaration_requirements(SurfaceDeclarationId::from_index(
            nocter_model::BuiltinType::COUNT + 3,
        ))
        .unwrap();
    assert_eq!(requirements.len(), 9);
    assert!(matches!(
        requirements[0],
        BoundRequirementKind::Capability { .. }
    ));
    assert!(matches!(
        requirements[1],
        BoundRequirementKind::Capability { .. }
    ));
    assert!(matches!(requirements[2], BoundRequirementKind::Copy(_)));
    assert!(matches!(
        requirements[3],
        BoundRequirementKind::TypeEquality { .. }
    ));
    assert!(matches!(
        requirements[4],
        BoundRequirementKind::Equality { .. }
    ));
    assert!(matches!(
        requirements[5],
        BoundRequirementKind::Ordering { .. }
    ));
    assert!(matches!(
        requirements[6],
        BoundRequirementKind::Index { .. }
    ));
    let BoundRequirementKind::Coercion { source, .. } = requirements[7] else {
        panic!("expected coercion requirement");
    };
    assert!(matches!(
        bound.kind(source),
        Some(BoundTypeKind::Borrow { .. })
    ));
    assert!(matches!(
        requirements[8],
        BoundRequirementKind::Expansion { .. }
    ));
}

#[test]
fn pattern_equalities_are_directed_refinements_and_cannot_retain_their_binder() {
    let mut sources = SourceMap::new();
    let app_manifest_id = add_source(&mut sources, "/app/index.nct", "");
    let std_manifest_id = add_source(&mut sources, "/std/index.nct", "");
    let app_id = add_source(
        &mut sources,
        "/app/index.nct",
        "struct Box<T> {}\ninstance Box<T> where T = i32 {}\n",
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
    assert!(matches!(
        bound
            .declaration_requirements(SurfaceDeclarationId::from_index(
                nocter_model::BuiltinType::COUNT + 1,
            ))
            .unwrap(),
        [BoundRequirementKind::BinderRefinement { .. }]
    ));

    let recursive_id = add_source(
        &mut sources,
        "/app/recursive.nct",
        "struct Box<T> {}\ninstance Box<T> where T = [T] {}\n",
    );
    let recursive = parse_source(&sources, recursive_id, ParseGoal::SourceFile);
    let error = bind(
        &sources,
        vec![
            package("workspace:app", "app", "/app/index.nct", &app_manifest),
            package("builtin:std", "std", "/std/index.nct", &std_manifest),
        ],
        vec![
            module("workspace:app", &[], "/app/recursive.nct", &recursive),
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
    .unwrap_err();
    assert!(matches!(
        error,
        TypeBindingError::Rule(violation)
            if violation.rule() == crate::TypeBindingRule::RecursiveBinderRefinement
    ));
}
