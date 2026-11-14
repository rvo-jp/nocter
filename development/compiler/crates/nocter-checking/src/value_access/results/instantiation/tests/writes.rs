//! Actual receiver environments after calls; logical metadata is not source capture admission.
use super::values::{bundle, id, insert_graph};
use super::*;
use crate::value_access::ir::LocalOrigin;
use crate::value_access::{CallKind, ValueSource, ValueSourceFlow, ValueSourceSlot, ValueSources};
use nocter_declarations::value_access::InstanceSchema;
use nocter_model::ValueField;

struct Case {
    program: Program,
    frame: CodeInstanceKey,
    before: ConcreteWitness,
    after: ConcreteWitness,
    evidence: Evidence,
    parent: TypeId,
}
fn completed_body(
    code: nocter_model::ValueCode,
    environment: TypeId,
    element: Option<TypeId>,
) -> ValueSources {
    let mut points = ArenaBuilder::new();
    let entry = points.insert(());
    let mut locals = ArenaBuilder::new();
    let environment_local = locals.insert(());
    let slot = ValueSourceSlot::Local(environment_local);
    let flow = ValueSourceFlow::test(
        entry,
        BTreeMap::from([(entry, 1)]),
        BTreeMap::from([(slot, id(0))]),
        BTreeMap::new(),
        BTreeMap::from([((entry, slot), vec![(0, id(2))])]),
        BTreeSet::from([entry]),
    );
    ValueSources::test(
        code,
        data::replacement_nodes(environment, element),
        BTreeMap::new(),
        BTreeSet::new(),
    )
    .with_test_flow(flow)
}

fn caller_graph(fixture: &Fixture, parent: TypeId, weak: bool) -> ValueSources {
    let program = &fixture.program;
    let mut locals = ArenaBuilder::new();
    let receiver = locals.insert(());
    let declared = program.codes().get(fixture.root).unwrap().parameters()[0];
    let formal = program
        .function_data(FunctionWitness {
            origin: Origin::Parameter(declared.into()),
            invocation: ValueInvocation::Edit,
        })
        .unwrap();
    let mut call = bundle(0, &[0], &[]);
    call.kind = CallKind::Value;
    call.invocation = ValueInvocation::Edit;
    ValueSources::test(
        fixture.root.into(),
        vec![
            (
                Some(formal),
                ValueSource::Input {
                    local: receiver,
                    origin: LocalOrigin::Parameter(declared),
                },
            ),
            (
                Some(formal),
                ValueSource::CallWrite {
                    call: fixture.sites[0],
                    argument: 0,
                    source: id(0),
                    path: ValuePath::default(),
                    weak,
                },
            ),
            (
                program.function_data(fixture.callbacks[1]),
                ValueSource::Function(fixture.callbacks[1]),
            ),
            (
                Some(parent),
                ValueSource::Aggregate(
                    vec![(ValueField::Tuple(0), id(0)), (ValueField::Tuple(1), id(2))].into(),
                ),
            ),
            (
                Some(parent),
                ValueSource::CallWrite {
                    call: fixture.sites[0],
                    argument: 0,
                    source: id(3),
                    path: ValuePath::new([nocter_model::ValueProjection::Tuple(0)]),
                    weak,
                },
            ),
        ],
        BTreeMap::from([(fixture.sites[0], vec![call])]),
        BTreeSet::new(),
    )
}

fn case(weak: bool) -> Case {
    let fixture = editing_fixture();
    let parent = fixture.receiver_carrier.unwrap();
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
    let before = program
        .bind_callable_data(fixture.captured, &environment)
        .unwrap();
    let changed = program
        .instantiate_components(
            environment.data(),
            [0, 1].map(|i| (ValueField::Tuple(i), leaves[1].clone())),
        )
        .unwrap();
    let after = program
        .bind_callable_data(fixture.captured, &changed)
        .unwrap();
    let frame = program
        .bind_instance(
            &program.concrete_witness(fixture.root.into()).unwrap(),
            [
                (fixture.root_formals[0], before.clone()),
                (
                    fixture.root_formals[1],
                    program.concrete_witness(fixture.callbacks[1]).unwrap(),
                ),
            ],
        )
        .unwrap();
    let caller = caller_graph(&fixture, parent, weak);
    let mut evidence = Evidence::default();
    insert_graph(&mut evidence, caller, &fixture.sites[..1]);
    insert_graph(
        &mut evidence,
        completed_body(
            before.code(),
            environment.data(),
            program.function_data(wrapper),
        ),
        &[],
    );
    Case {
        program: fixture.program,
        frame,
        before,
        after,
        evidence,
        parent,
    }
}

#[test]
fn a_value_receiver_uses_the_actual_callees_completed_environment_instead_of_its_old_snapshot() {
    let mut case = case(false);
    assert_eq!(case.before.witness(), case.after.witness());
    assert_ne!(case.before, case.after);
    assert_eq!(
        value(
            &case.program,
            &case.evidence,
            &case.frame,
            id(0),
            &mut budget()
        )
        .unwrap(),
        case.before
    );
    for _ in 0..2 {
        assert_eq!(
            value(
                &case.program,
                &case.evidence,
                &case.frame,
                id(1),
                &mut budget()
            )
            .unwrap(),
            case.after
        );
        let mut tiny = WorkBudget::new(NonZeroU64::new(1).unwrap());
        assert_eq!(
            value(&case.program, &case.evidence, &case.frame, id(1), &mut tiny),
            Err(InstantiationFailure::WorkLimit)
        );
    }
    case.evidence.value_sources.remove(&case.before.code());
    assert_eq!(
        value(
            &case.program,
            &case.evidence,
            &case.frame,
            id(1),
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
}

#[test]
fn weak_receiver_updates_retain_the_previous_context_as_a_real_alternative() {
    let case = case(true);
    assert_eq!(
        value(
            &case.program,
            &case.evidence,
            &case.frame,
            id(1),
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
}

#[test]
fn receiver_components_rebuild_their_carrier_without_changing_an_independent_sibling() {
    let mut case = case(false);
    let parent = data(
        &case.program,
        &case.evidence,
        &case.frame,
        id(4),
        case.parent,
        &mut budget(),
    )
    .unwrap();
    let InstanceSchema::Function(selected) = parent
        .component(&case.program, ValueField::Tuple(0))
        .unwrap()
        .schema(&case.program)
        .unwrap()
    else {
        panic!("updated receiver")
    };
    assert_eq!(selected, case.after);
    let InstanceSchema::Function(sibling) = parent
        .component(&case.program, ValueField::Tuple(1))
        .unwrap()
        .schema(&case.program)
        .unwrap()
    else {
        panic!("independent sibling")
    };
    case.evidence.value_sources.remove(&case.before.code());
    let key = Key::Value(
        case.frame.clone(),
        id(4),
        ValuePath::new([nocter_model::ValueProjection::Tuple(1)]),
    );
    assert_eq!(
        Query::new(&case.program, &case.evidence, &mut budget()).solve(&key),
        Ok(Fact::Witness(sibling))
    );
    assert_eq!(
        data(
            &case.program,
            &case.evidence,
            &case.frame,
            id(4),
            case.parent,
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
}
