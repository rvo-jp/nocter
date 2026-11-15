//! Logical data transport over one exact Unit; these are not executable source receipts.
use super::*;
use crate::value_access::accepted::{SourceUnit, Unit};
use nocter_model::AccessLocalId;
use nocter_test_support::ValueSourceFixture;
use std::sync::Arc;
mod calls;
mod roots;

struct Fixture {
    unit: Arc<Unit>,
    frame: CodeInstanceKey,
    local: AccessLocalId,
    old: DataInstanceKey,
    supplied: DataInstanceKey,
}
impl Fixture {
    fn new() -> Self {
        let source = ValueSourceFixture::new(
            "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc work(take action: func(): i32): func(): i32 { action^ }",
        );
        let unit = Arc::new(SourceUnit::prepare(source.syntax()).unwrap().into_unit());
        let program = unit.program();
        let code = |name| {
            program
                .resolve(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let work = code("work");
        let parameter = program.codes().get(work).unwrap().parameters()[0];
        let frame = program
            .bind_code(work.into(), [(parameter.into(), code("first").into())])
            .unwrap();
        let body = unit.code_body(frame.code()).unwrap().typed().body();
        let local = body.inputs().explicit()[0].local;
        let data = body.locals().get(local).unwrap().data;
        let old = program.instantiate_data(data, &frame).unwrap();
        let second = program.concrete_witness(code("second").into()).unwrap();
        let supplied = program.instantiate_function(data, &second).unwrap();
        assert_eq!(old.data(), supplied.data());
        assert_ne!(old, supplied);
        Self {
            unit,
            frame,
            local,
            old,
            supplied,
        }
    }
    fn projection<'a, 'work>(
        &'a self,
        work: &'work mut WorkBudget,
    ) -> ResourceDataProjection<'a, 'work> {
        let at = Location {
            point: self
                .unit
                .code_body(self.frame.code())
                .unwrap()
                .graph()
                .entry(),
            action: 0,
        };
        self.unit
            .project_resource_data(&self.frame, at, work)
            .unwrap()
    }
    fn contract(&self) -> crate::value_access::flow::resources::Payload {
        crate::value_access::flow::resources::payload(self.unit.program(), self.old.data()).unwrap()
    }
}
fn budget(units: u64) -> WorkBudget {
    WorkBudget::new(std::num::NonZeroU64::new(units).unwrap())
}

#[test]
fn descriptor_reception_preserves_supplied_contexts_beside_excluded_components() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-excluded-result-components.nct"
    ));
    let runtime = source
        .replace("const ANSWER: i32 = calculate()", "")
        .replace("const func", "func");
    let source = ValueSourceFixture::new(&runtime);
    let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
    let program = unit.program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let relay = code("relay");
    let bundle = code("bundle");
    let input = program.codes().get(relay).unwrap().parameters()[0].into();
    let body = unit.code_body(relay.into()).unwrap();
    let at = body
        .graph()
        .points()
        .iter()
        .find_map(|(point, block)| {
            block
                .actions
                .iter()
                .position(|action| matches!(action.operation, Operation::StageReturn(_)))
                .map(|action| Location { point, action })
        })
        .unwrap();
    let call = body.typed().body().expressions().iter().find_map(|(id, expression)| matches!(expression.kind, ExpressionKind::Call { target: nocter_model::ValueFunctionOrigin::Declaration(target), .. } if target == bundle).then_some(id)).unwrap();
    let target = body.typed().result_data();
    let callable = program
        .component(target, ValueField::Tuple(1))
        .unwrap()
        .data;
    let Some(PayloadSchema::Function(witness)) = program.data_schema(callable) else {
        panic!("contextual callable");
    };
    assert!(
        program.concrete_witness(witness).is_err(),
        "the receiving descriptor cannot manufacture the supplied environment"
    );
    let mut contexts = Vec::new();
    for name in ["identity", "increment"] {
        let frame = program
            .bind_code(relay.into(), [(input, code(name).into())])
            .unwrap();
        let called = unit
            .instantiate_invocation(
                &frame,
                nocter_model::ValueApplication {
                    owner: relay.into(),
                    call,
                },
                &mut budget(100_000),
            )
            .unwrap();
        let supplied = unit
            .instantiate_result_contract(&called, &mut budget(100_000))
            .unwrap();
        let mut work = budget(100_000);
        let projection = unit.project_resource_data(&frame, at, &mut work).unwrap();
        let received = projection.receiving(supplied.clone(), target).unwrap();
        assert_eq!(received.data(), target);
        assert!(supplied.has_same_representation(program, &received));
        contexts.push(received);
    }
    assert_ne!(contexts[0], contexts[1]);
}

#[test]
fn an_invalid_original_action_cannot_construct_a_resource_data_projection() {
    let fixture = Fixture::new();
    let mut work = budget(1_000);
    let at = Location {
        point: fixture
            .unit
            .code_body(fixture.frame.code())
            .unwrap()
            .graph()
            .entry(),
        action: usize::MAX,
    };
    assert_eq!(
        fixture
            .unit
            .project_resource_data(&fixture.frame, at, &mut work)
            .err(),
        Some(ResourceDataFailure::Integrity)
    );
}

#[test]
fn ordered_replacement_uses_the_supplied_value_instead_of_the_frames_type_default() {
    assert_replacement(false);
}
#[test]
fn scoped_replacement_keeps_the_retired_payload_separate_from_the_supplied_payload() {
    assert_replacement(true);
}
fn assert_replacement(scoped: bool) {
    let fixture = Fixture::new();
    let mut work = budget(10_000);
    let mut projection = fixture.projection(&mut work);
    projection.values.insert(
        Holder::Local(fixture.local),
        Binding::Value(fixture.old.clone()),
    );
    projection
        .values
        .insert(Holder::Return, Binding::Value(fixture.supplied.clone()));
    let destination = Holder::Local(fixture.local).place();
    let source = Holder::Return.place();
    let contract = fixture.contract();
    let mut steps = if scoped {
        vec![ResourceStep::ReplaceReferent {
            destination: destination.clone(),
            source,
            retired: contract,
            payload: contract,
        }]
    } else {
        vec![
            ResourceStep::Dispose {
                source: destination.clone(),
                payload: contract,
            },
            ResourceStep::Move {
                source,
                destination: destination.clone(),
                payload: contract,
            },
        ]
    };
    steps.push(ResourceStep::Dispose {
        source: destination,
        payload: contract,
    });
    let projected = projection.steps(&steps).unwrap();
    let ([
        ResourceStep::Dispose {
            payload: previous, ..
        },
        ResourceStep::Move {
            payload: supplied, ..
        },
        ResourceStep::Dispose {
            payload: final_value,
            ..
        },
    ]
    | [
        ResourceStep::ReplaceReferent {
            retired: previous,
            payload: supplied,
            ..
        },
        ResourceStep::Dispose {
            payload: final_value,
            ..
        },
    ]) = projected.as_slice()
    else {
        panic!("original ordered replacement and final cleanup")
    };
    let program = fixture.unit.program();
    assert_eq!(
        previous.instance(),
        &fixture
            .old
            .function_representation(program)
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        supplied.instance(),
        &fixture
            .supplied
            .function_representation(program)
            .unwrap()
            .unwrap()
    );
    assert_eq!(final_value, supplied);
    assert_ne!(previous, supplied);
}

#[test]
fn contextual_retirement_without_a_write_target_cannot_use_the_input_snapshot() {
    let fixture = Fixture::new();
    let mut work = budget(10_000);
    let mut projection = fixture.projection(&mut work);
    assert_eq!(
        projection.retired(&Holder::Local(fixture.local).place(), fixture.old.data()),
        Err(INVALID)
    );
    assert!(projection.values.is_empty());
}

#[test]
fn failure_discards_intermediate_bindings_without_discarding_prior_stage_bindings() {
    let fixture = Fixture::new();
    let mut work = budget(10_000);
    let mut projection = fixture.projection(&mut work);
    projection.values.insert(
        Holder::Local(fixture.local),
        Binding::Value(fixture.old.clone()),
    );
    projection
        .values
        .insert(Holder::Return, Binding::Value(fixture.supplied.clone()));
    let future = fixture
        .unit
        .code_body(fixture.frame.code())
        .unwrap()
        .typed()
        .body()
        .expressions()
        .iter()
        .next()
        .unwrap()
        .0;
    let result = projection.steps(&[
        ResourceStep::Move {
            source: Holder::Return.place(),
            destination: Holder::Local(fixture.local).place(),
            payload: fixture.contract(),
        },
        ResourceStep::Dispose {
            source: Holder::Expression(future).place(),
            payload: fixture.contract(),
        },
    ]);
    assert!(matches!(
        result,
        Err(ResourceDataFailure::Instantiation(
            InstantiationFailure::Unresolved
        ))
    ));
    assert!(
        matches!(projection.values.get(&Holder::Local(fixture.local)), Some(Binding::Value(key)) if key == &fixture.old)
    );
    assert!(
        matches!(projection.values.get(&Holder::Return), Some(Binding::Value(key)) if key == &fixture.supplied)
    );
}

#[test]
fn projection_work_failure_leaves_the_same_unit_available_for_retry() {
    let fixture = Fixture::new();
    let steps = [ResourceStep::Dispose {
        source: Holder::Local(fixture.local).place(),
        payload: fixture.contract(),
    }];
    let mut exhausted = budget(1);
    assert_eq!(
        fixture.projection(&mut exhausted).steps(&steps),
        Err(ResourceDataFailure::Instantiation(
            InstantiationFailure::WorkLimit
        ))
    );
    let mut healthy = budget(10_000);
    assert_eq!(
        fixture
            .projection(&mut healthy)
            .steps(&steps)
            .unwrap()
            .len(),
        1
    );
}
