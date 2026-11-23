//! Header scheduling and agreement use actual source contexts, not a second typed program.
use super::module_contracts::inventory;
use super::module_scopes::{declaration, source};
use super::*;
use crate::{DeclarationContractRule, SurfaceDeclarationKind as Kind};
use nocter_compile_input::ModuleSourceKind::{Implementation, Root};
use nocter_declarations::value_access::{
    CodeKind, DeclarationId, HeaderPosition, TypeExpressionSource,
};
use nocter_model::{ConstantValue, FrozenData, FrozenNode};

fn count(value: u64) -> FrozenData {
    FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Integer(
        value.into(),
    ))])
    .unwrap()
}
fn advance(
    scoped: &mut ScopedModuleSources<'_>,
    id: impl Into<DeclarationId>,
) -> Result<HeaderProgress, ModuleSourcesFailure> {
    scoped.advance_header_with_values(id, |_| None)
}

#[test]
fn paired_generic_headers_reserve_one_ordered_slot_list() {
    let index = "see ./body.nct\nfunc decode<T, const N: usize>(take item: T): T";
    let body = "see ./index.nct\nfunc decode<T, const N: usize>(take item: T): T { item^ }";
    let mut scoped = inventory(
        &[
            ("/app/index.nct", Root, index),
            ("/app/body.nct", Implementation, body),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let code_at = |path| {
        let raw = declaration(&scoped, path, Kind::Function, "decode");
        scoped.declaration(raw).unwrap()
    };
    let DeclarationId::Code(code) = code_at("/app/index.nct") else {
        panic!("callable contract");
    };
    assert_eq!(code_at("/app/body.nct"), DeclarationId::Code(code));
    let program = scoped.program();
    let slots = program.generic_parameter_ids(code).unwrap();
    assert_eq!(slots.len(), 2);
    assert!(
        slots
            .iter()
            .all(|&slot| program.generic_parameter(slot).unwrap().owner() == code)
    );
    assert!(matches!(
        advance(&mut scoped, code),
        Err(ModuleSourcesFailure::Access(Failure::Syntax(diagnostics)))
            if diagnostics[0].code() == "E0291"
    ));

    let reordered = "see ./index.nct\nfunc decode<const N: usize, T>(take item: T): T { item^ }";
    let Err(ModuleSourcesFailure::Contract(diagnostic)) = inventory(
        &[
            ("/app/index.nct", Root, index),
            ("/app/body.nct", Implementation, reordered),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair() else {
        panic!("a different generic order must reject the pair");
    };
    assert_eq!(diagnostic.rule(), DeclarationContractRule::MismatchedBody);
}

#[test]
fn scalar_enum_pairs_keep_copy_policy_and_original_variant_audience() {
    let mut scoped = inventory(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nenum State { ready\nwaiting }\nenum Secret\ncopy enum Token"),
            ("/app/body.nct", Implementation, "see ./index.nct\nenum Secret { hidden }\ncopy enum Token { issued }\nfunc inside(): void {}"),
            ("/app/observer.nct", Implementation, "see ./index.nct\nfunc observer(): void {}"),
        ],
        &[(0, 0, 1), (1, 0, 0), (2, 0, 0)],
        true,
    ).pair().unwrap().reserve_names().unwrap();
    let identify = |scoped: &ScopedModuleSources<'_>, path, kind, name| {
        scoped
            .declaration(declaration(scoped, path, kind, name))
            .unwrap()
    };
    let inside = identify(&scoped, "/app/body.nct", Kind::Function, "inside");
    let observer = identify(&scoped, "/app/observer.nct", Kind::Function, "observer");
    let ids = scoped.declaration_ids().collect::<Vec<_>>();
    for id in ids {
        assert_eq!(advance(&mut scoped, id).unwrap(), HeaderProgress::Ready);
    }
    let ready = scoped.seal().unwrap();
    let current = ready.current().unwrap();
    current.verify_header_values(|_| None).unwrap();
    let DeclarationId::Code(inside) = inside else {
        panic!("inside body")
    };
    let DeclarationId::Code(observer) = observer else {
        panic!("observer body")
    };
    let inside = current.for_code(inside).unwrap();
    let observer = current.for_code(observer).unwrap();
    let program = ready.program();
    for (name, variant, copy, public) in [
        ("State", "waiting", true, true),
        ("Secret", "hidden", false, false),
        ("Token", "issued", true, false),
    ] {
        let data = program
            .type_identity_named_in(observer.lookup_scope(), name)
            .expect("visible nominal header");
        let nominal = program.nominal_identity(data).unwrap();
        assert_eq!(program.enumeration(nominal).unwrap().copyable(), copy);
        let selected = inside.enum_variant(data, variant).unwrap();
        let external = observer.enum_variant(data, variant).unwrap();
        assert_eq!(selected.id, external.id);
        assert!(selected.accessible);
        assert_eq!(external.accessible, public);
    }
}

#[test]
fn destructor_header_binds_its_original_body_and_implicit_receiver() {
    let mut scoped = inventory(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Handle"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Handle { value: i32 }\nimpl Handle { drop {} }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let DeclarationId::Nominal(owner) = scoped
        .declaration(declaration(
            &scoped,
            "/app/index.nct",
            Kind::Struct,
            "Handle",
        ))
        .unwrap()
    else {
        panic!("nominal owner");
    };
    let codes = scoped.code_ids().collect::<Vec<_>>();
    let &[drop] = codes.as_slice() else {
        panic!("one lifecycle code")
    };
    assert_eq!(advance(&mut scoped, owner).unwrap(), HeaderProgress::Ready);
    assert_eq!(advance(&mut scoped, drop).unwrap(), HeaderProgress::Ready);
    let sealed = scoped.seal().unwrap();
    let current = sealed.current().unwrap();
    current.verify_header_values(|_| None).unwrap();
    let code = sealed.program().codes().get(drop).unwrap();
    assert_eq!(code.kind(), CodeKind::Destructor(owner));
    assert!(code.name().is_none());
    let source = current.code_source(drop).unwrap();
    assert!(source.body().is_some());
    assert!(source.parameter_origin(0, None).is_some());
    assert!(source.parameter_type(0).is_none());
}

#[test]
fn destructor_duplication_is_reported_across_separate_impl_fragments() {
    let headers = inventory(
        &[
            ("/app/index.nct", Root, "see ./body.nct\nstruct Handle"),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Handle {}\nimpl Handle { drop {} }\nimpl Handle { drop {} }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    ).pair().unwrap().reserve_names();
    let Err(ModuleSourcesFailure::Access(Failure::SourceRole(error))) = headers else {
        panic!("duplicate destructor must have a source diagnostic: {headers:?}");
    };
    assert_eq!(error.rule(), SourceRoleRule::DuplicateDestructor);
}

#[test]
fn scalar_enum_headers_reject_empty_repeated_or_second_representations() {
    for (text, code) in [
        ("enum State {}", "E0200"),
        ("enum State { idle\nidle }", "E0241"),
    ] {
        let mut scoped = inventory(&[("/app/index.nct", Root, text)], &[], false)
            .pair()
            .unwrap()
            .reserve_names()
            .unwrap();
        let id = scoped
            .declaration(declaration(&scoped, "/app/index.nct", Kind::Enum, "State"))
            .unwrap();
        let Err(ModuleSourcesFailure::Access(Failure::Headers(errors))) = advance(&mut scoped, id)
        else {
            panic!("invalid enum schema");
        };
        assert_eq!(errors[0].source().code(), code);
        assert_eq!(
            errors[0].source().notes().len(),
            usize::from(code == "E0241")
        );
    }
    let completed = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nenum State { idle }",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nenum State { idle }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair();
    let Err(ModuleSourcesFailure::Contract(diagnostic)) = completed else {
        panic!("closed enum completed again");
    };
    assert_eq!(
        diagnostic.rule(),
        DeclarationContractRule::DuplicateRepresentation
    );
}

#[test]
fn scalar_enum_variants_and_type_members_share_one_namespace() {
    let mut scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nenum State { idle }",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nimpl State { func idle(): Self { loop {} } }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let ids = scoped
        .declaration_ids()
        .chain(scoped.code_ids().map(DeclarationId::Code))
        .collect::<Vec<_>>();
    for id in ids {
        assert_eq!(advance(&mut scoped, id).unwrap(), HeaderProgress::Ready);
    }
    let sealed = scoped.seal();
    let Err(ModuleSourcesFailure::Access(Failure::SourceRole(error))) = sealed else {
        panic!(
            "type member and variant names must be distinct: {:?}",
            sealed.err()
        );
    };
    assert_eq!(error.rule(), SourceRoleRule::EnumVariantMemberNameConflict);
}

#[test]
fn paired_headers_agree_on_nominal_identity_before_payload_readiness() {
    for (kind, root_header, body_header, expected_rule) in [
        (
            Kind::Function,
            "func relay(value: Data): Data",
            "func relay(value: Data): Data { value }",
            DeclarationContractRule::MismatchedBody,
        ),
        (
            Kind::Constant,
            "const relay: Data",
            "const relay: Data = Data { value: 42 }",
            DeclarationContractRule::MismatchedCompileTimeInitializer,
        ),
        (
            Kind::Static,
            "static relay: Data",
            "static relay: Data = Data { value: 42 }",
            DeclarationContractRule::MismatchedCompileTimeInitializer,
        ),
    ] {
        for same in [false, true] {
            let root_text = format!("see ./body.nct\nsee ./data.nct\n{root_header}");
            let body_text = format!(
                "see ./index.nct\nsee ./{}.nct\n{body_header}",
                if same { "data" } else { "other" }
            );
            let mut scoped = inventory(
                &[
                    ("/app/index.nct", Root, &root_text),
                    ("/app/body.nct", Implementation, &body_text),
                    (
                        "/app/data.nct",
                        Implementation,
                        "struct Data { value: i32 }",
                    ),
                    (
                        "/app/other.nct",
                        Implementation,
                        "struct Data { value: i64 }",
                    ),
                ],
                &[
                    (0, 0, 1),
                    (0, 1, 2),
                    (1, 0, 0),
                    (1, 1, if same { 2 } else { 3 }),
                ],
                true,
            )
            .pair()
            .unwrap()
            .reserve_names()
            .unwrap();
            let id = scoped
                .declaration(declaration(&scoped, "/app/index.nct", kind, "relay"))
                .unwrap();
            let early = scoped.program();
            if same {
                let data = scoped
                    .declaration(declaration(&scoped, "/app/data.nct", Kind::Struct, "Data"))
                    .unwrap();
                // Header agreement establishes the nominal identity, not a materialized value.
                // Constant/static execution still needs the complete payload through its own stage.
                assert_eq!(
                    advance(&mut scoped, id).unwrap(),
                    HeaderProgress::Ready,
                    "{kind:?}"
                );
                assert!(scoped.program().header_ready(id));
                assert!(!scoped.program().header_ready(data));
                assert_eq!(advance(&mut scoped, data).unwrap(), HeaderProgress::Ready);
                assert_eq!(advance(&mut scoped, id).unwrap(), HeaderProgress::Ready);
                assert!(scoped.program().header_ready(id));
                assert!(scoped.program().preserves_headers(&early));
                assert!(!early.header_ready(id));
            } else {
                for _ in 0..2 {
                    let Err(ModuleSourcesFailure::Contract(error)) = advance(&mut scoped, id)
                    else {
                        panic!("resolved mismatch")
                    };
                    assert_eq!(error.rule(), expected_rule);
                    assert_eq!(
                        error.source().primary().source(),
                        scoped.paired().inventory().sources()
                            [source(&scoped, "/app/body.nct").index()]
                        .syntax()
                        .source()
                    );
                    assert!(!scoped.program().header_ready(id));
                    assert!(scoped.program().codes().is_empty());
                }
            }
        }
    }
}

fn count_sources() -> ScopedModuleSources<'static> {
    inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nsee ./root_count.nct\nfunc values(): [i32; size()]",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nsee ./body_count.nct\nfunc values(): [i32; size()] { [1, 2] }",
            ),
            (
                "/app/root_count.nct",
                Implementation,
                "const func size(): usize { 2 }",
            ),
            (
                "/app/body_count.nct",
                Implementation,
                "const func size(): usize { 3 }",
            ),
        ],
        &[(0, 0, 1), (0, 1, 2), (1, 0, 0), (1, 1, 3)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap()
}

#[test]
fn all_physical_count_suppliers_are_requested_and_unequal_results_do_not_define_the_header() {
    let mut scoped = count_sources();
    let id = scoped
        .declaration(declaration(
            &scoped,
            "/app/index.nct",
            Kind::Function,
            "values",
        ))
        .unwrap();
    let ids = scoped.header_expression_ids().collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    let HeaderProgress::RequireValues(requested) = advance(&mut scoped, id).unwrap() else {
        panic!("physical counts")
    };
    assert_eq!(requested.as_ref(), ids);
    let program = scoped.program();
    let mut helpers = Vec::new();
    for expression in &ids {
        let physical = scoped.source_for_code(*expression).unwrap();
        let tree = scoped.paired().inventory().sources()[physical.index()].syntax();
        let syntax = bound(scoped.paired().inventory().source_map(), tree);
        let body = scoped.code_source(*expression).unwrap().body().unwrap();
        assert_eq!(
            syntax.text_at(tree.node(body).unwrap().range()),
            Some("size()")
        );
        helpers.push(
            program
                .declaration_in(
                    scoped.scope(physical).unwrap(),
                    Namespace::Value,
                    program.symbols().get("size").unwrap(),
                )
                .unwrap(),
        );
        assert_eq!(
            advance(&mut scoped, *expression).unwrap(),
            HeaderProgress::Ready
        );
        assert_eq!(
            scoped.program().codes().get(*expression).unwrap().kind(),
            CodeKind::TypeExpression(TypeExpressionSource {
                declaration: id,
                position: HeaderPosition::Result
            })
        );
    }
    assert_ne!(helpers[0], helpers[1]);
    let malformed = FrozenData::new(vec![FrozenNode::Scalar(ConstantValue::Bool(true))]).unwrap();
    assert!(matches!(
        scoped.advance_header_with_values(id, |_| Some(&malformed)),
        Err(ModuleSourcesFailure::Access(Failure::InconsistentSyntax))
    ));
    assert!(!scoped.program().header_ready(id));
    let two = count(2);
    let three = count(3);
    assert_eq!(
        scoped
            .advance_header_with_values(id, |code| (code == ids[0]).then_some(&two))
            .unwrap(),
        HeaderProgress::RequireValues(vec![ids[1]].into())
    );
    let Err(ModuleSourcesFailure::Contract(error)) = scoped
        .advance_header_with_values(id, |code| Some(if code == ids[0] { &two } else { &three }))
    else {
        panic!("unequal count results")
    };
    assert_eq!(error.rule(), DeclarationContractRule::MismatchedBody);
    assert!(!scoped.program().header_ready(id));
    assert_eq!(
        scoped
            .advance_header_with_values(id, |_| Some(&two))
            .unwrap(),
        HeaderProgress::Ready
    );
    let DeclarationId::Code(code) = id else {
        panic!("code")
    };
    let program = scoped.program();
    let result = program.codes().get(code).unwrap().result();
    assert!(
        matches!(program.data_schema(result), Some(nocter_declarations::value_access::PayloadSchema::Array(array)) if array.length() == 2)
    );
    // These supplied literals qualify the header protocol only, never source-body evaluation.
    assert!(!program.headers_complete());
}

#[test]
fn selected_public_field_counts_map_to_complete_representation_slots() {
    let mut scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nstruct Bag { shown: [i32; 2] }",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nstruct Bag { hidden: [bool; 1]\nshown: [i32; 2] }",
            ),
        ],
        &[(0, 0, 1), (1, 0, 0)],
        true,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let record = scoped
        .declaration(declaration(&scoped, "/app/index.nct", Kind::Struct, "Bag"))
        .unwrap();
    let ids = scoped.header_expression_ids().collect::<Vec<_>>();
    assert_eq!(ids.len(), 3);
    let mut inputs = std::collections::BTreeMap::new();
    let mut positions = Vec::new();
    for id in &ids {
        let physical = scoped.source_for_code(*id).unwrap();
        let tree = scoped.paired().inventory().sources()[physical.index()].syntax();
        let syntax = bound(scoped.paired().inventory().source_map(), tree);
        let body = scoped.code_source(*id).unwrap().body().unwrap();
        let value = syntax
            .text_at(tree.node(body).unwrap().range())
            .unwrap()
            .parse::<u64>()
            .unwrap();
        inputs.insert(*id, count(value));
        assert_eq!(advance(&mut scoped, *id).unwrap(), HeaderProgress::Ready);
        let CodeKind::TypeExpression(slot) = scoped.program().codes().get(*id).unwrap().kind()
        else {
            panic!("type expression")
        };
        assert_eq!(slot.declaration, record);
        positions.push(slot.position);
    }
    assert_eq!(
        positions
            .iter()
            .filter(|p| **p == HeaderPosition::Field(1))
            .count(),
        2
    );
    assert_eq!(
        positions
            .iter()
            .filter(|p| **p == HeaderPosition::Field(0))
            .count(),
        1
    );
    assert_eq!(
        scoped
            .advance_header_with_values(record, |code| inputs.get(&code))
            .unwrap(),
        HeaderProgress::Ready
    );
    let DeclarationId::Nominal(id) = record else {
        panic!("record")
    };
    let slots = scoped.field_ids(id).unwrap();
    assert_eq!(slots.len(), 2);
    assert!(scoped.program().fields().get(slots[0]).is_some());
    assert!(scoped.program().fields().get(slots[1]).is_some());
}

#[test]
fn parameter_collisions_in_only_the_implementation_scope_keep_both_actual_locations() {
    let mut scoped = inventory(
        &[
            (
                "/app/index.nct",
                Root,
                "see ./body.nct\nfunc relay(arg: i32): i32",
            ),
            (
                "/app/body.nct",
                Implementation,
                "see ./index.nct\nsee ./helpers.nct\nfunc relay(arg: i32): i32 { arg }",
            ),
            ("/app/helpers.nct", Implementation, "func arg(): i32 { 42 }"),
        ],
        &[(0, 0, 1), (1, 0, 0), (1, 1, 2)],
        false,
    )
    .pair()
    .unwrap()
    .reserve_names()
    .unwrap();
    let id = scoped
        .declaration(declaration(
            &scoped,
            "/app/index.nct",
            Kind::Function,
            "relay",
        ))
        .unwrap();
    let Err(ModuleSourcesFailure::Access(Failure::Headers(errors))) = advance(&mut scoped, id)
    else {
        panic!("local collision")
    };
    assert_eq!(errors.len(), 1);
    let inventory = scoped.paired().inventory();
    assert_eq!(
        errors[0].source().primary().source(),
        inventory.sources()[source(&scoped, "/app/body.nct").index()]
            .syntax()
            .source()
    );
    assert_eq!(
        errors[0].source().notes()[0].origin().source(),
        inventory.sources()[source(&scoped, "/app/helpers.nct").index()]
            .syntax()
            .source()
    );
    assert!(scoped.program().codes().is_empty());
}

#[test]
fn visible_nominal_fields_compare_resolved_identities_at_their_complete_slots() {
    for same in [false, true] {
        let body = if same {
            "see ./index.nct\nsee ./root_type.nct\nstruct Bag { hidden: bool\nshown: Data }"
        } else {
            "see ./index.nct\nsee ./body_type.nct\nstruct Bag { hidden: bool\nshown: Data }"
        };
        let mut scoped = inventory(
            &[
                (
                    "/app/index.nct",
                    Root,
                    "see ./body.nct\nsee ./root_type.nct\nstruct Bag { shown: Data }",
                ),
                ("/app/body.nct", Implementation, body),
                (
                    "/app/root_type.nct",
                    Implementation,
                    "struct Data { value: i32 }",
                ),
                (
                    "/app/body_type.nct",
                    Implementation,
                    "struct Data { value: i64 }",
                ),
            ],
            &[
                (0, 0, 1),
                (0, 1, 2),
                (1, 0, 0),
                (1, 1, if same { 2 } else { 3 }),
            ],
            false,
        )
        .pair()
        .unwrap()
        .reserve_names()
        .unwrap();
        let bag = scoped
            .declaration(declaration(&scoped, "/app/index.nct", Kind::Struct, "Bag"))
            .unwrap();
        if same {
            let data = scoped
                .declaration(declaration(
                    &scoped,
                    "/app/root_type.nct",
                    Kind::Struct,
                    "Data",
                ))
                .unwrap();
            assert_eq!(
                advance(&mut scoped, bag).unwrap(),
                HeaderProgress::RequireHeaders(vec![data].into())
            );
            assert_eq!(advance(&mut scoped, data).unwrap(), HeaderProgress::Ready);
            assert_eq!(advance(&mut scoped, bag).unwrap(), HeaderProgress::Ready);
        } else {
            let Err(ModuleSourcesFailure::Contract(error)) = advance(&mut scoped, bag) else {
                panic!("resolved field mismatch")
            };
            assert_eq!(
                error.rule(),
                DeclarationContractRule::MismatchedRepresentation
            );
            assert!(scoped.program().nominals().is_empty() && scoped.program().fields().is_empty());
        }
    }
}

#[test]
fn supplied_header_counts_cannot_create_a_reusable_module_query_without_source_guards() {
    let mut scoped = count_sources();
    let mut seen = std::collections::HashSet::new();
    let mut pending = scoped
        .code_ids()
        .map(DeclarationId::Code)
        .chain(scoped.declaration_ids())
        .filter(|id| seen.insert(*id))
        .collect::<Vec<_>>();
    let two = count(2);
    while !pending.is_empty() {
        let before = pending.len();
        pending.retain(|id| {
            !matches!(
                scoped
                    .advance_header_with_values(*id, |_| Some(&two))
                    .unwrap(),
                HeaderProgress::Ready
            )
        });
        assert!(pending.len() < before);
    }
    let current = scoped.seal().unwrap();
    assert!(matches!(
        current.query(),
        Err(ModuleSourcesFailure::Access(Failure::MissingValueEvidence))
    ));
}
