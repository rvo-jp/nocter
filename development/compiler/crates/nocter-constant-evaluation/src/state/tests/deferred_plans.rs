//! Deferred bodies retain fully linked and immutable invocation boundaries.
use super::*;

#[test]
fn deferred_call_boundaries_validate_before_bodies_and_extend_atomically() {
    let (root, target) = code_ids();
    let mut root_plan = literal();
    root_plan.points = root_plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: target.into(),
            inputs: Box::new([]),
        };
        point
    });
    let mut target_plan = literal();
    target_plan.initializer = false;
    let contracts = BTreeMap::from([
        (root.into(), CallContract::from_plan(&root_plan).unwrap()),
        (
            target.into(),
            CallContract::from_plan(&target_plan).unwrap(),
        ),
    ]);
    let mut program = Program::linked(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        contracts,
        BTreeMap::from([(root.into(), root_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    assert!(!program.plans.contains_key(&target.into()));
    let original = program.plans[&root.into()].clone();
    let mut wrong = target_plan.clone();
    wrong.result = 1;
    wrong.slots = vec![Slot {
        data: 1,
        storage: Storage::Owned,
    }]
    .into();
    assert_eq!(
        program.extend(ProgramExtension {
            types: vec![Type::Bool],
            plans: BTreeMap::from([(target.into(), wrong)]),
            ..ProgramExtension::default()
        }),
        Err(Rule::InvalidPlan)
    );
    assert_eq!(program.types.len(), 1);
    assert!(!program.plans.contains_key(&target.into()));
    assert!(Arc::ptr_eq(&original, &program.plans[&root.into()]));
    assert_eq!(
        program.extend(ProgramExtension {
            contracts: BTreeMap::from([(
                target.into(),
                CallContract::from_plan(&target_plan).unwrap()
            )]),
            ..ProgramExtension::default()
        }),
        Err(Rule::InvalidPlan)
    );
    program
        .extend(ProgramExtension {
            plans: BTreeMap::from([(target.into(), target_plan)]),
            ..ProgramExtension::default()
        })
        .unwrap();
    assert!(Arc::ptr_eq(&original, &program.plans[&root.into()]));
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn deferred_loading_preserves_exact_keys_caches_bodies_and_recovers_after_preparation_failure() {
    struct Bodies {
        pending: BTreeMap<CodeInstance, Plan>,
        calls: Vec<CodeInstance>,
        fail: bool,
        omit: bool,
    }
    impl PlanSource for Bodies {
        fn load(&mut self, code: CodeInstance) -> Result<ProgramExtension, PlanFailure> {
            self.calls.push(code);
            if std::mem::take(&mut self.fail) {
                return Err(Rule::PlanWorkLimit.into());
            }
            if std::mem::take(&mut self.omit) {
                return Ok(ProgramExtension {
                    types: vec![Type::Bool],
                    ..ProgramExtension::default()
                });
            }
            Ok(ProgramExtension {
                plans: BTreeMap::from([(
                    code,
                    self.pending.remove(&code).ok_or(Rule::UnavailablePlan)?,
                )]),
                ..ProgramExtension::default()
            })
        }
    }
    let (root, target) = code_ids();
    let mut root_plan = literal();
    root_plan.points = root_plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target: target.into(),
            inputs: Box::new([]),
        };
        point
    });
    let mut target_plan = literal();
    target_plan.initializer = false;
    let dormant = CodeInstance {
        source: target.into(),
        instance: 1,
    };
    let program = Program::linked(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (root.into(), CallContract::from_plan(&root_plan).unwrap()),
            (
                target.into(),
                CallContract::from_plan(&target_plan).unwrap(),
            ),
            (dormant, CallContract::from_plan(&target_plan).unwrap()),
        ]),
        BTreeMap::from([(root.into(), root_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap_err()
            .rule,
        Rule::UnavailablePlan
    );
    assert_eq!(budget.storage_used(), 0);
    let mut source = Bodies {
        pending: BTreeMap::from([(target.into(), target_plan.clone()), (dormant, target_plan)]),
        calls: Vec::new(),
        fail: true,
        omit: true,
    };
    {
        let mut executor = Executor::with_source(program, &mut source, &mut budget, 0);
        let error = executor.evaluate_initializer(root).unwrap_err();
        assert_eq!(error.rule, Rule::PlanWorkLimit);
        assert_eq!(error.site.unwrap().code, root.into());
        assert!(error.trace.is_empty());
        assert_eq!(
            executor.evaluate_initializer(root).unwrap_err().rule,
            Rule::InvalidPlan
        );
        for _ in 0..2 {
            assert_eq!(
                executor.evaluate_initializer(root).unwrap(),
                Value::integer(42)
            );
        }
    }
    assert_eq!(source.calls, [target.into(), target.into(), target.into()]);
    assert!(source.pending.contains_key(&dormant));
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn every_failed_append_phase_rolls_back_types_contracts_bodies_and_statics_before_retry() {
    let (root, initializer) = code_ids();
    let static_id = nocter_model::ValueStatic { initializer };
    let batch = || ProgramExtension {
        types: vec![Type::Bool],
        contracts: BTreeMap::from([(root.into(), CallContract::from_plan(&literal()).unwrap())]),
        plans: BTreeMap::from([(root.into(), literal())]),
        statics: BTreeMap::from([(
            static_id,
            Static {
                data: 0,
                value: Value::integer(42),
            },
        )]),
    };
    let mut program = Program::new(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::new(),
        BTreeMap::new(),
    )
    .unwrap();
    for stage in 0..4 {
        let mut invalid = batch();
        match stage {
            0 => invalid.types = vec![Type::Record(vec![1].into())],
            1 => invalid.statics.get_mut(&static_id).unwrap().value = Value::boolean(false),
            2 => invalid.contracts.get_mut(&root.into()).unwrap().result = 1,
            3 => {
                let body = invalid.plans.get_mut(&root.into()).unwrap();
                body.points = body.points.map(|_, point| {
                    let mut point = point.clone();
                    point.actions[0].operation = Operation::Literal {
                        destination: 0,
                        value: Value::boolean(false),
                    };
                    point
                });
            }
            _ => unreachable!(),
        }
        assert_eq!(
            program.extend(invalid),
            Err(Rule::InvalidPlan),
            "phase {stage}"
        );
        assert_eq!(
            program.types,
            [Type::Integer(nocter_model::BuiltinType::I32)]
        );
        assert!(program.contracts.is_empty());
        assert!(program.plans.is_empty());
        assert!(program.statics.is_empty());
    }
    program.extend(batch()).unwrap();
    validation::validate(&program).unwrap();
    let mut budget = Budget::new(Limits::default());
    assert_eq!(
        Executor::new(&program, &mut budget, 0)
            .evaluate_initializer(root)
            .unwrap(),
        Value::integer(42)
    );
    assert_eq!(program.statics[&static_id].value, Value::integer(42));
    assert_eq!(budget.storage_used(), 0);
}

#[test]
fn deferred_failure_preserves_inner_operation_specialization_and_call_path() {
    struct FailedBody {
        failure: PlanFailure,
        expected: CodeInstance,
    }
    impl PlanSource for FailedBody {
        fn load(&mut self, code: CodeInstance) -> Result<ProgramExtension, PlanFailure> {
            assert_eq!(code, self.expected);
            Err(self.failure)
        }
    }
    let (root, target) = code_ids();
    let target = CodeInstance {
        source: target.into(),
        instance: 37,
    };
    let mut root_plan = literal();
    root_plan.points = root_plan.points.map(|_, point| {
        let mut point = point.clone();
        point.actions[0].operation = Operation::Call {
            destination: 0,
            target,
            inputs: Box::new([]),
        };
        point
    });
    let caller = Site {
        code: root.into(),
        point: root_plan.entry,
        action: 0,
    };
    let mut target_plan = literal();
    target_plan.initializer = false;
    let inner = Site {
        code: target.source,
        point: target_plan.entry,
        action: 0,
    };
    let program = Program::linked(
        [Type::Integer(nocter_model::BuiltinType::I32)],
        BTreeMap::from([
            (root.into(), CallContract::from_plan(&root_plan).unwrap()),
            (target, CallContract::from_plan(&target_plan).unwrap()),
        ]),
        BTreeMap::from([(root.into(), root_plan)]),
        BTreeMap::new(),
    )
    .unwrap();
    for rule in [Rule::UnresolvedContext, Rule::PlanWorkLimit] {
        for foreign in [false, true] {
            let mut source = FailedBody {
                failure: PlanFailure {
                    rule,
                    site: Some(if foreign { caller } else { inner }),
                },
                expected: target,
            };
            let mut budget = Budget::new(Limits::default());
            let error = Executor::with_source(program.clone(), &mut source, &mut budget, 0)
                .evaluate_initializer(root)
                .unwrap_err();
            if foreign {
                assert_eq!(error.rule, Rule::InvalidPlan);
                assert_eq!(error.site, Some(caller));
            } else {
                assert_eq!(error.rule, rule);
                assert_eq!(error.site, Some(inner));
                assert_eq!(error.instance.as_deref(), Some(&target));
                assert_eq!(&*error.trace, &[caller]);
            }
            assert_eq!(budget.storage_used(), 0);
        }
    }
}
