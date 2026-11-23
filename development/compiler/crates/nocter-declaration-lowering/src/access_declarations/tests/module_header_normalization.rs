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
