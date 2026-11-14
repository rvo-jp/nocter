//! Completed structural input metadata; these descriptors do not open source capture admission.
use super::values::{bundle, id, insert_graph};
use super::*;
use crate::value_access::{
    CallKind, ValueSource, ValueSourceFlow, ValueSourceInput, ValueSourceSlot, ValueSources,
    ir::LocalOrigin,
};
use nocter_declarations::value_access::{
    FunctionConstraint, FunctionInput, InstanceSchema, PayloadSchema,
};
use nocter_model::{ValueField, ValueInputOperation as Op, ValueProjection};

struct Case {
    program: Program,
    frame: CodeInstanceKey,
    update: ValueClosure,
    call: AccessExpressionId,
    carrier: TypeId,
    parent: TypeId,
    wrapper: TypeId,
    before: DataInstanceKey,
    after: DataInstanceKey,
    leaf: ConcreteWitness,
}
fn witness(closure: ValueClosure) -> FunctionWitness {
    FunctionWitness {
        origin: Origin::Closure(closure),
        invocation: ValueInvocation::Read,
    }
}
fn wrap(
    program: &Program,
    wrapper: TypeId,
    formal: TypeId,
    callback: FunctionWitness,
) -> DataInstanceKey {
    let PayloadSchema::Function(selected) = program.data_schema(wrapper).unwrap() else {
        panic!("wrapper")
    };
    let environment = program
        .instantiate_function(formal, &program.concrete_witness(callback).unwrap())
        .unwrap();
    let actual = program.bind_callable_data(selected, &environment).unwrap();
    program.instantiate_function(wrapper, &actual).unwrap()
}
impl Case {
    fn new() -> Self {
        let (sources, tree) = input(
            "func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc creator(take action: func(): i32): void {}",
        );
        let syntax = bound(&sources, &tree);
        let declarations =
            nocter_declaration_lowering::access_declarations::Query::prepare(syntax).unwrap();
        let context = declarations.bind(syntax).unwrap();
        let header = context.program();
        let code = |name| {
            header
                .resolve_code(header.symbols().get(name).unwrap())
                .unwrap()
        };
        let creator = code("creator");
        let callbacks: [FunctionWitness; 2] = [code("first").into(), code("second").into()];
        let parameter = header.codes().get(creator).unwrap().parameters()[0];
        let formal = header
            .function_data(FunctionWitness {
                origin: Origin::Parameter(parameter.into()),
                invocation: ValueInvocation::Read,
            })
            .unwrap();
        let result = header.data_type(BuiltinType::I32).unwrap();
        let void = header.data_type(BuiltinType::Void).unwrap();
        let contract = FunctionConstraint {
            invocation: ValueInvocation::Read,
            inputs: Box::new([]),
            result,
        };
        let mut body = header.body_types(creator).unwrap();
        let wrapper_code = body.reserve_closure().unwrap();
        let wrapper = body
            .closure(wrapper_code, formal, contract.clone())
            .unwrap();
        let carrier = body.tuple(&[wrapper, wrapper]).unwrap();
        let parent = body.tuple(&[carrier, result]).unwrap();
        let outer = body.reserve_closure().unwrap();
        body.closure(outer, carrier, contract.clone()).unwrap();
        let update = body.reserve_closure().unwrap();
        body.closure(
            update,
            carrier,
            FunctionConstraint {
                inputs: vec![FunctionInput {
                    data: carrier,
                    operation: Op::Edit,
                }]
                .into(),
                result: void,
                ..contract
            },
        )
        .unwrap();
        let program = body.finish();
        let leaves = callbacks.map(|callback| wrap(&program, wrapper, formal, callback));
        let before = program
            .instantiate_components(
                carrier,
                [
                    (ValueField::Tuple(0), leaves[0].clone()),
                    (ValueField::Tuple(1), leaves[1].clone()),
                ],
            )
            .unwrap();
        let after = program
            .instantiate_components(
                carrier,
                [0, 1].map(|i| (ValueField::Tuple(i), leaves[1].clone())),
            )
            .unwrap();
        let frame = program
            .bind_instance(
                &program.bind_callable_data(witness(outer), &before).unwrap(),
                [],
            )
            .unwrap();
        let InstanceSchema::Function(leaf) = leaves[1].schema(&program).unwrap() else {
            panic!("wrapper")
        };
        let mut expressions = ArenaBuilder::new();
        Self {
            program,
            frame,
            update,
            call: expressions.insert(()),
            carrier,
            parent,
            wrapper,
            before,
            after,
            leaf,
        }
    }
    fn caller(&self, weak: bool) -> ValueSources {
        let mut locals = ArenaBuilder::new();
        let mut call = bundle(1, &[1, 0], &[]);
        call.kind = CallKind::Value;
        ValueSources::test(
            self.frame.code(),
            vec![
                (
                    Some(self.carrier),
                    ValueSource::Input {
                        local: locals.insert(()),
                        origin: LocalOrigin::Capture,
                    },
                ),
                (
                    self.program.function_data(witness(self.update)),
                    ValueSource::Closure {
                        closure: self.update,
                        environment: id(0),
                    },
                ),
                (
                    Some(self.carrier),
                    ValueSource::CallWrite {
                        call: self.call,
                        argument: 1,
                        source: id(0),
                        path: ValuePath::default(),
                        weak,
                    },
                ),
                (
                    Some(self.wrapper),
                    ValueSource::Select {
                        source: id(2),
                        path: ValuePath::new([ValueProjection::Tuple(0)]),
                    },
                ),
                (
                    Some(self.wrapper),
                    ValueSource::Select {
                        source: id(2),
                        path: ValuePath::new([ValueProjection::Tuple(1)]),
                    },
                ),
                (
                    self.program.data_type(BuiltinType::I32),
                    ValueSource::Opaque,
                ),
                (
                    Some(self.parent),
                    ValueSource::Aggregate(
                        vec![(ValueField::Tuple(0), id(0)), (ValueField::Tuple(1), id(5))].into(),
                    ),
                ),
                (
                    Some(self.parent),
                    ValueSource::CallWrite {
                        call: self.call,
                        argument: 1,
                        source: id(6),
                        path: ValuePath::new([ValueProjection::Tuple(0)]),
                        weak,
                    },
                ),
                (
                    Some(self.wrapper),
                    ValueSource::Select {
                        source: id(7),
                        path: ValuePath::new([
                            ValueProjection::Tuple(0),
                            ValueProjection::Tuple(0),
                        ]),
                    },
                ),
            ],
            BTreeMap::from([(self.call, vec![call])]),
            BTreeSet::new(),
        )
    }
    fn body(&self) -> ValueSources {
        let mut locals = ArenaBuilder::new();
        let environment = locals.insert(());
        let target = locals.insert(());
        let mut points = ArenaBuilder::new();
        let entry = points.insert(());
        let flow = ValueSourceFlow::test(
            entry,
            BTreeMap::from([(entry, 1)]),
            BTreeMap::from([
                (ValueSourceSlot::Local(environment), id(0)),
                (ValueSourceSlot::Local(target), id(1)),
            ]),
            BTreeMap::new(),
            BTreeMap::from([((entry, ValueSourceSlot::Local(target)), vec![(0, id(4))])]),
            BTreeSet::from([entry]),
            vec![
                ValueSourceInput {
                    source: id(0),
                    operation: Op::Read,
                },
                ValueSourceInput {
                    source: id(1),
                    operation: Op::Edit,
                },
            ]
            .into(),
        );
        ValueSources::test(
            ValueCode::Closure(self.update),
            vec![
                (
                    Some(self.carrier),
                    ValueSource::Input {
                        local: environment,
                        origin: LocalOrigin::Capture,
                    },
                ),
                (
                    Some(self.carrier),
                    ValueSource::Input {
                        local: target,
                        origin: LocalOrigin::ClosureParameter,
                    },
                ),
                (
                    Some(self.wrapper),
                    ValueSource::Select {
                        source: id(0),
                        path: ValuePath::new([ValueProjection::Tuple(1)]),
                    },
                ),
                (
                    Some(self.carrier),
                    ValueSource::Aggregate(
                        vec![(ValueField::Tuple(0), id(2)), (ValueField::Tuple(1), id(2))].into(),
                    ),
                ),
                (
                    Some(self.carrier),
                    ValueSource::Replace {
                        source: id(1),
                        value: id(3),
                        path: ValuePath::default(),
                        weak: false,
                    },
                ),
            ],
            BTreeMap::new(),
            BTreeSet::new(),
        )
        .with_test_flow(flow)
    }
    fn evidence(&self, weak: bool) -> Evidence {
        let mut evidence = Evidence::default();
        insert_graph(&mut evidence, self.caller(weak), &[self.call]);
        insert_graph(&mut evidence, self.body(), &[]);
        evidence
    }
}

#[test]
fn edited_carriers_and_selected_callable_components_follow_the_same_completed_input() {
    let case = Case::new();
    let evidence = case.evidence(false);
    assert_ne!(case.before, case.after);
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            case.carrier,
            &mut budget()
        )
        .unwrap(),
        case.after
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(3), &mut budget()).unwrap(),
        case.leaf
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(4), &mut budget()).unwrap(),
        case.leaf
    );
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(0),
            case.carrier,
            &mut budget()
        )
        .unwrap(),
        case.before
    );
}

#[test]
fn edited_subcarriers_rebuild_the_parent_without_reusing_its_written_prefix_in_the_callee() {
    let case = Case::new();
    let evidence = case.evidence(false);
    let scalar = case
        .program
        .instantiate_closed_data(case.program.data_type(BuiltinType::I32).unwrap())
        .unwrap();
    let expected = case
        .program
        .instantiate_components(
            case.parent,
            [
                (ValueField::Tuple(0), case.after.clone()),
                (ValueField::Tuple(1), scalar),
            ],
        )
        .unwrap();
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(7),
            case.parent,
            &mut budget()
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(8), &mut budget()).unwrap(),
        case.leaf
    );
}

#[test]
fn weak_carrier_writes_join_selected_components_without_choosing_a_whole_context() {
    let case = Case::new();
    let evidence = case.evidence(true);
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            case.carrier,
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(3), &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(4), &mut budget()).unwrap(),
        case.leaf
    );
}

#[test]
fn completed_carrier_queries_fail_without_body_or_schema_evidence_and_recover_after_work_failure() {
    let case = Case::new();
    let mut evidence = case.evidence(false);
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            case.carrier,
            &mut WorkBudget::new(NonZeroU64::new(1).unwrap())
        ),
        Err(InstantiationFailure::WorkLimit)
    );
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            case.carrier,
            &mut budget()
        )
        .unwrap(),
        case.after
    );
    let wrong = case.program.data_type(BuiltinType::Bool).unwrap();
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            wrong,
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
    evidence
        .value_sources
        .remove(&ValueCode::Closure(case.update));
    assert_eq!(
        data(
            &case.program,
            &evidence,
            &case.frame,
            id(2),
            case.carrier,
            &mut budget()
        ),
        Err(InstantiationFailure::Unresolved)
    );
    assert_eq!(
        value(&case.program, &evidence, &case.frame, id(3), &mut budget()),
        Err(InstantiationFailure::Unresolved)
    );
}
