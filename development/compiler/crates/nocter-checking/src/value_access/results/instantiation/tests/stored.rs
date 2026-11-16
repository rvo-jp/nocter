//! Sparse stored definitions close loops without erasing different occurrence contexts.
use super::values::{id, insert_graph};
use super::*;
use crate::value_access::{
    ValueSourceBoundary, ValueSourceFlow, ValueSourceSelection, ValueSourceSlot, ValueSources,
};
use nocter_model::{AccessFlowPointId, ValueField, ValueProjection};

#[test]
fn stored_queries_close_anchored_backedges() {
    assert_loop_context(false);
}

#[test]
fn stored_queries_retain_different_data_contexts_at_a_join() {
    assert_loop_context(true);
}

fn assert_loop_context(update: bool) {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let (environment, leaves) = creations::mixed_environment(&fixture, outer, wrapper);
    let selected = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let frame = program.bind_instance(&selected, []).unwrap();
    let mut locals = ArenaBuilder::new();
    let local = locals.insert(());
    let slot = ValueSourceSlot::Local(local);
    let replaced = program
        .instantiate_components(
            environment.data(),
            [0, 1].map(|index| (ValueField::Tuple(index), leaves[1].clone())),
        )
        .unwrap();
    let (positions, points) = loop_positions(slot, update);
    let graph = ValueSources::test(
        frame.code(),
        data::replacement_nodes(environment.data(), program.function_data(wrapper)),
        BTreeMap::new(),
        BTreeSet::new(),
    )
    .with_test_flow(positions);
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, graph, &[]);
    assert_eq!(
        Query::new(program, &evidence, &mut budget()).solve(&Key::CompletedInput(
            frame.clone(),
            local,
            ValuePath::default(),
            environment.data(),
        )),
        Ok(Fact::Empty),
        "a known body with no normal endpoints contributes no final context"
    );
    let at = ValueSourceSelection {
        boundary: ValueSourceBoundary {
            point: points[1],
            action: 0,
        },
        slot,
        path: ValuePath::default(),
    };
    let expected = if update {
        expected_loop_domain(program, environment.data(), &leaves)
    } else {
        environment.clone()
    };
    assert_eq!(
        stored(
            program,
            &evidence,
            &frame,
            &at,
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        expected
    );
    let component = ValueSourceSelection {
        path: ValuePath::new([ValueProjection::Tuple(1)]),
        ..at.clone()
    };
    assert_stored_retry(program, &evidence, &frame, &component, &leaves[1]);
    let after = ValueSourceSelection {
        boundary: ValueSourceBoundary {
            point: points[2],
            action: 1,
        },
        ..at
    };
    assert_eq!(
        stored(
            program,
            &evidence,
            &frame,
            &after,
            environment.data(),
            &mut budget()
        )
        .unwrap(),
        if update {
            replaced.clone()
        } else {
            environment.clone()
        }
    );
}

fn expected_loop_domain(
    program: &Program,
    data: TypeId,
    leaves: &[DataInstanceKey],
) -> DataInstanceKey {
    let choices = leaves.iter().map(|key| {
        let nocter_declarations::value_access::InstanceSchema::Function(actual) =
            key.schema(program).unwrap()
        else {
            panic!("original leaf")
        };
        actual
    });
    let first = program
        .instantiate_function_alternatives(leaves[0].data(), choices)
        .unwrap();
    program
        .instantiate_components(
            data,
            [
                (ValueField::Tuple(0), first),
                (ValueField::Tuple(1), leaves[1].clone()),
            ],
        )
        .unwrap()
}

fn loop_positions(
    slot: ValueSourceSlot,
    update: bool,
) -> (ValueSourceFlow, [AccessFlowPointId; 3]) {
    let mut arena = ArenaBuilder::new();
    let points = std::array::from_fn(|_| arena.insert(()));
    let mut writes = BTreeMap::from([((points[0], slot), vec![(0, id(0))])]);
    if update {
        writes.insert((points[2], slot), vec![(0, id(2))]);
    }
    (
        ValueSourceFlow::test(
            points[0],
            BTreeMap::from([(points[0], 1), (points[1], 0), (points[2], 1)]),
            BTreeMap::new(),
            BTreeMap::from([
                (points[1], vec![points[0], points[2]]),
                (points[2], vec![points[1]]),
            ]),
            writes,
            BTreeSet::new(),
            Box::new([]),
        ),
        points,
    )
}

#[test]
fn completed_inputs_join_actual_final_contexts_and_never_choose_one_return() {
    let fixture = fixture_with_shape(1, false);
    let program = &fixture.program;
    let Origin::Closure(outer) = fixture.captured.origin else {
        unreachable!()
    };
    let &wrapper = fixture.supporting.keys().next().unwrap();
    let wrapper = FunctionWitness {
        origin: Origin::Closure(wrapper),
        invocation: ValueInvocation::Read,
    };
    let (environment, leaves) = creations::mixed_environment(&fixture, outer, wrapper);
    let actual = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let frame = program.bind_instance(&actual, []).unwrap();
    let replaced = program
        .instantiate_components(
            environment.data(),
            [0, 1].map(|index| (ValueField::Tuple(index), leaves[1].clone())),
        )
        .unwrap();
    let mut locals = ArenaBuilder::new();
    let local = locals.insert(());
    let slot = ValueSourceSlot::Local(local);
    for same_context in [false, true] {
        let mut points = ArenaBuilder::new();
        let [entry, left, right] = std::array::from_fn(|_| points.insert(()));
        let mut writes = BTreeMap::from([((left, slot), vec![(0, id(2))])]);
        if same_context {
            writes.insert((right, slot), vec![(0, id(2))]);
        }
        let positions = ValueSourceFlow::test(
            entry,
            BTreeMap::from([(entry, 0), (left, 1), (right, 1)]),
            BTreeMap::from([(slot, id(0))]),
            BTreeMap::from([(left, vec![entry]), (right, vec![entry])]),
            writes,
            BTreeSet::from([left, right]),
            Box::new([]),
        );
        let graph = ValueSources::test(
            frame.code(),
            data::replacement_nodes(environment.data(), program.function_data(wrapper)),
            BTreeMap::new(),
            BTreeSet::new(),
        )
        .with_test_flow(positions);
        let mut evidence = Evidence::default();
        insert_graph(&mut evidence, graph, &[]);
        let expected = if same_context {
            Ok(replaced.clone())
        } else {
            Err(InstantiationFailure::Unresolved)
        };
        for _ in 0..2 {
            assert_eq!(
                completed_input(
                    program,
                    &evidence,
                    &frame,
                    local,
                    environment.data(),
                    &mut budget()
                ),
                expected
            );
            let mut tiny = WorkBudget::new(NonZeroU64::new(1).unwrap());
            assert_eq!(
                completed_input(
                    program,
                    &evidence,
                    &frame,
                    local,
                    environment.data(),
                    &mut tiny
                ),
                Err(InstantiationFailure::WorkLimit)
            );
        }
    }
}

fn assert_stored_retry(
    program: &Program,
    evidence: &Evidence,
    frame: &CodeInstanceKey,
    selected: &ValueSourceSelection,
    expected: &nocter_declarations::value_access::DataInstanceKey,
) {
    assert_eq!(
        stored(
            program,
            evidence,
            frame,
            selected,
            expected.data(),
            &mut budget()
        )
        .unwrap(),
        *expected
    );
    assert_eq!(
        stored(
            program,
            evidence,
            frame,
            selected,
            expected.data(),
            &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        stored(
            program,
            evidence,
            frame,
            selected,
            expected.data(),
            &mut budget()
        )
        .unwrap(),
        *expected
    );
}
