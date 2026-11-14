//! Distinct preparation and post-operation metadata, without executable source-write permission.
use super::*;
use crate::value_access::{
    ArgumentAction, ExpressionKind, ValueSource, ValueSourceFlow, ValueSourceId, ValueSourceRead,
    ValueSources,
};
use nocter_model::AccessExpressionId;
use std::collections::BTreeSet;

struct CallFixture {
    unit: Unit,
    frame: CodeInstanceKey,
    at: Location,
    prepare: Location,
    call: AccessExpressionId,
    first: DataInstanceKey,
    second: DataInstanceKey,
}
impl CallFixture {
    fn new() -> Self {
        let source = ValueSourceFixture::new(
            "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc observe(edit action: func(): i32): void { let _ = action() }\nfunc invoke(take action: func(): i32): void { observe(action^) }",
        );
        let unit = SourceUnit::prepare(source.syntax()).unwrap().into_unit();
        let program = unit.program();
        let code = |name| {
            program
                .resolve_code(program.symbols().get(name).unwrap())
                .unwrap()
        };
        let invoke = code("invoke");
        let parameter = program.codes().get(invoke).unwrap().parameters()[0];
        let first = program.concrete_witness(code("first").into()).unwrap();
        let second = program.concrete_witness(code("second").into()).unwrap();
        let frame = program
            .bind_instance(
                &program.concrete_witness(invoke.into()).unwrap(),
                [(parameter.into(), first.clone())],
            )
            .unwrap();
        let body = unit.code_body(frame.code()).unwrap();
        let actions = body
            .graph()
            .points()
            .iter()
            .flat_map(|(point, block)| {
                block
                    .actions
                    .iter()
                    .enumerate()
                    .map(move |(action, event)| (Location { point, action }, &event.operation))
            })
            .collect::<Vec<_>>();
        let (at, call) = actions
            .iter()
            .find_map(|(at, operation)| match operation {
                Operation::CommitCall(call) => Some((*at, *call)),
                _ => None,
            })
            .unwrap();
        let prepare = actions
            .iter()
            .find_map(|(at, operation)| {
                matches!(operation,
            Operation::PrepareArgument { call: selected, argument: 0 } if *selected == call)
                .then_some(*at)
            })
            .unwrap();
        assert_eq!(prepare.point, at.point);
        let ExpressionKind::Call { arguments, .. } =
            &body.typed().body().expressions().get(call).unwrap().kind
        else {
            panic!("call")
        };
        assert!(matches!(
            arguments[0].action,
            ArgumentAction::ReserveTransfer { .. }
        ));
        let data = body
            .typed()
            .body()
            .expressions()
            .get(arguments[0].value)
            .unwrap()
            .ty
            .normal()
            .unwrap()
            .data();
        let first = program.instantiate_function(data, &first).unwrap();
        let second = program.instantiate_function(data, &second).unwrap();
        Self {
            unit,
            frame,
            at,
            prepare,
            call,
            first,
            second,
        }
    }
    fn holder(&self) -> Holder {
        Holder::Temporary {
            call: self.call,
            index: 0,
        }
    }
    fn dispose(&self) -> ResourceStep {
        ResourceStep::Dispose {
            source: self.holder().place(),
            payload: flow::resources::payload(self.unit.program(), self.first.data()).unwrap(),
        }
    }
    fn evidence(&self) -> Evidence {
        let body = self.unit.code_body(self.frame.code()).unwrap();
        let nodes = [&self.first, &self.second].map(|key| {
            let InstanceSchema::Function(witness) = key.schema(self.unit.program()).unwrap() else {
                panic!("callable")
            };
            (Some(key.data()), ValueSource::Function(witness.witness()))
        });
        let flow = ValueSourceFlow::test(
            body.graph().entry(),
            body.graph()
                .points()
                .iter()
                .map(|(point, block)| (point, block.actions.len()))
                .collect(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::from([(
                (
                    self.at.point,
                    ValueSourceSlot::Argument {
                        call: self.call,
                        index: 0,
                    },
                ),
                vec![
                    (self.prepare.action, ValueSourceId::test(0)),
                    (self.at.action, ValueSourceId::test(1)),
                ],
            )]),
            BTreeSet::new(),
            Box::new([]),
        );
        let graph = ValueSources::test(
            self.frame.code(),
            nodes.into(),
            BTreeMap::new(),
            BTreeSet::new(),
        )
        .with_test_flow(flow);
        let mut evidence = Evidence::default();
        evidence.value_sources.insert(self.frame.code(), graph);
        evidence
    }
    fn projection<'a, 'work>(
        &'a self,
        evidence: &'a Evidence,
        at: Location,
        work: &'work mut WorkBudget,
    ) -> ResourceDataProjection<'a, 'work> {
        let mut projection = ResourceDataProjection::new(
            self.unit.program(),
            evidence,
            self.unit.code_body(self.frame.code()),
            &self.frame,
            at,
            work,
        )
        .unwrap();
        projection
            .values
            .insert(self.holder(), Binding::Value(self.first.clone()));
        projection
    }
}

#[test]
fn original_lent_argument_definitions_preserve_the_frozen_bundle_and_add_a_completed_write() {
    let fixture = CallFixture::new();
    let graph = fixture.unit.value_sources(fixture.frame.code()).unwrap();
    let flow = graph.flow().unwrap();
    let slot = ValueSourceSlot::Argument {
        call: fixture.call,
        index: 0,
    };
    let before = flow
        .read(
            ValueSourceBoundary {
                point: fixture.at.point,
                action: fixture.at.action,
            },
            slot,
        )
        .unwrap();
    let ValueSourceRead::Value(before) = before else {
        panic!("prepared")
    };
    assert_eq!(graph.call(fixture.call).unwrap()[0].arguments[0], before);
    let ValueSourceRead::Value(after) = flow
        .read(
            ValueSourceBoundary {
                point: fixture.at.point,
                action: fixture.at.action + 1,
            },
            slot,
        )
        .unwrap()
    else {
        panic!("completed")
    };
    assert_ne!(before, after);
    assert!(
        matches!(graph.node(after).unwrap().value, ValueSource::CallWrite { call, argument: 0, source, weak: false, .. } if call == fixture.call && source == before)
    );
}

#[test]
fn temporary_cleanup_uses_completed_context_while_preparation_and_interruption_keep_the_entry() {
    let fixture = CallFixture::new();
    let evidence = fixture.evidence();
    let mut work = budget(10_000);
    let prepared = super::super::super::instantiate_prepared_data(
        fixture.unit.program(),
        &evidence,
        fixture.unit.code_body(fixture.frame.code()),
        &fixture.frame,
        &mut work,
    )
    .unwrap();
    assert_eq!(prepared.get(fixture.call, 0), Some(&fixture.first));
    let steps = [fixture.dispose()];
    let mut projection = fixture.projection(&evidence, fixture.at, &mut work);
    let before = projection.steps(&steps).unwrap();
    let after = projection.after_steps(&steps).unwrap();
    let ResourceStep::Dispose {
        payload: before, ..
    } = &before[0]
    else {
        panic!("dispose")
    };
    let ResourceStep::Dispose { payload: after, .. } = &after[0] else {
        panic!("dispose")
    };
    let program = fixture.unit.program();
    assert_eq!(
        before.instance(),
        &fixture
            .first
            .function_representation(program)
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        after.instance(),
        &fixture
            .second
            .function_representation(program)
            .unwrap()
            .unwrap()
    );
    assert_ne!(before, after);
    let mut interrupted = fixture.projection(&evidence, fixture.prepare, &mut work);
    assert_eq!(
        interrupted.after_steps(&steps).unwrap()[0],
        ResourceStep::Dispose {
            source: fixture.holder().place(),
            payload: before.clone()
        }
    );
}

#[test]
fn completed_temporary_work_failure_restores_the_entry_binding_and_can_retry() {
    let fixture = CallFixture::new();
    let evidence = fixture.evidence();
    let mut tiny = budget(1);
    let mut projection = fixture.projection(&evidence, fixture.at, &mut tiny);
    assert_eq!(
        projection.after_steps(&[fixture.dispose()]),
        Err(Failure::Instantiation(InstantiationFailure::WorkLimit))
    );
    assert!(
        matches!(projection.values.get(&fixture.holder()), Some(Binding::Value(key)) if key == &fixture.first)
    );
    let mut healthy = budget(10_000);
    assert!(
        fixture
            .projection(&evidence, fixture.at, &mut healthy)
            .after_steps(&[fixture.dispose()])
            .is_ok()
    );
}
