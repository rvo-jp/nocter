//! Sparse stored definitions close loops without erasing different occurrence contexts.
use super::values::{id, insert_graph};
use super::*;
use crate::value_access::{
    ValueSourceBoundary, ValueSourceFlow, ValueSourceSelection, ValueSourceSlot, ValueSources,
};
use nocter_model::{AccessFlowPointId, ValueField, ValueProjection};

#[test]
fn stored_queries_close_anchored_backedges_and_reject_different_carrier_contexts_at_a_join() {
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
    let slot = ValueSourceSlot::Local(locals.insert(()));
    let replaced = program
        .instantiate_components(
            environment.data(),
            [0, 1].map(|index| (ValueField::Tuple(index), leaves[1].clone())),
        )
        .unwrap();
    for update in [false, true] {
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
        let at = ValueSourceSelection {
            boundary: ValueSourceBoundary {
                point: points[1],
                action: 0,
            },
            slot,
            path: ValuePath::default(),
        };
        let expected = if update {
            Err(InstantiationFailure::Unresolved)
        } else {
            Ok(environment.clone())
        };
        assert_eq!(
            stored(
                program,
                &evidence,
                &frame,
                &at,
                environment.data(),
                &mut budget()
            ),
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
        ),
        points,
    )
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
