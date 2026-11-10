//! Replays CFG choices and emitted bit recipes, not Nocter value computation or native execution.
use super::*;
use lowering::flags::BitValue;
use nocter_checking::value_access::flow::{Terminator, resources::Holder};

fn write(words: &mut [u64], writes: &[BitValue]) {
    for &bit in writes {
        let word = &mut words[bit.word()];
        if bit.value() {
            *word |= bit.mask();
        } else {
            *word &= !bit.mask();
        }
    }
}

/// Follow selected CFG edges and normal completions; only initialization bits are executed.
fn trace(program: &lowering::Program, choices: &[bool]) -> Vec<Step> {
    let id = program.instance_program().root();
    let checked = program.instance_program().body(id).unwrap();
    let body = &program.bodies()[&id];
    let mut words = body.flags().initial_words().to_vec();
    let mut point = checked.graph().entry();
    let mut choices = choices.iter();
    let mut steps = Vec::new();
    for _ in 0..1000 {
        let block = checked.graph().points().get(point).unwrap();
        for (action, _) in block.actions.iter().enumerate() {
            let at = Location { point, action };
            let matching = body.actions()[&at]
                .alternatives()
                .iter()
                .filter(|alternative| {
                    alternative
                        .flags()
                        .tests()
                        .iter()
                        .all(|&bit| (words[bit.word()] & bit.mask() != 0) == bit.value())
                })
                .collect::<Vec<_>>();
            assert_eq!(matching.len(), 1, "{at:?}: {words:?}");
            let selected = matching[0];
            steps.extend_from_slice(selected.before());
            write(&mut words, selected.flags().before_writes());
            steps.extend_from_slice(selected.after());
            write(&mut words, selected.flags().after_writes());
        }
        point = match block.terminator {
            Terminator::Jump(next) => next,
            Terminator::Branch { yes, no, .. } => {
                if *choices.next().expect("branch choice") {
                    yes
                } else {
                    no
                }
            }
            Terminator::Return => {
                assert!(choices.next().is_none(), "unused branch choices");
                return steps;
            }
            Terminator::Unreachable => panic!("unexpected divergent path"),
        };
    }
    panic!("flag trace did not reach return");
}

#[test]
fn straight_line_ownership_needs_no_initialization_flag_bank() {
    let program = lower_source("func run(take value: error): error { value^ }");
    let body = &program.bodies()[&program.instance_program().root()];
    assert!(body.flags().initial_words().is_empty());
    for alternative in body
        .actions()
        .values()
        .flat_map(lowering::Action::alternatives)
    {
        assert!(alternative.flags().tests().is_empty());
        assert!(alternative.flags().before_writes().is_empty());
        assert!(alternative.flags().after_writes().is_empty());
    }
    trace(&program, &[]);
}

#[test]
fn partial_field_retirement_selects_the_correct_cleanup_after_each_branch() {
    let program = lower_source(
        "struct Pair { first: error\nsecond: error }\nfunc run(take pair: Pair, flag: bool): void {\nif flag { let saved = pair.first^ }\n}",
    );
    let partial = trace(&program, &[true]);
    assert_eq!(
        partial
            .iter()
            .filter(|step| matches!(step, Step::Resource(ResourceStep::ReleaseContainer { .. })))
            .count(),
        1
    );
    assert!(!partial.iter().any(|step| matches!(step, Step::Resource(ResourceStep::Dispose { payload, .. }) if payload.disposal() == Disposal::Aggregate)));
    let complete = trace(&program, &[false]);
    assert_eq!(complete.iter().filter(|step| matches!(step, Step::Resource(ResourceStep::Dispose { payload, .. }) if payload.disposal() == Disposal::Aggregate)).count(), 1);
    assert!(
        !complete
            .iter()
            .any(|step| matches!(step, Step::Resource(ResourceStep::ReleaseContainer { .. })))
    );
}

#[test]
fn a_present_container_with_no_remaining_fields_is_distinct_from_an_absent_owner() {
    let program = lower_source(
        "struct Pair { first: error\nsecond: error }\nfunc run(take pair: Pair, flag: bool): void {\nif flag { let first = pair.first^\nlet second = pair.second^ } else { drop pair }\n}",
    );
    let empty_container = trace(&program, &[true]);
    let absent = trace(&program, &[false]);
    assert_eq!(
        empty_container
            .iter()
            .filter(|step| matches!(step, Step::Resource(ResourceStep::ReleaseContainer { .. })))
            .count(),
        1
    );
    assert!(
        !absent
            .iter()
            .any(|step| matches!(step, Step::Resource(ResourceStep::ReleaseContainer { .. })))
    );
}

#[test]
fn restoration_resets_the_field_flag_before_the_next_loop_iteration() {
    let program = lower_source(
        "struct Pair { first: i32\nsecond: error }\nfunc run(take pair: Pair, take rounds: i32, take choose: bool): void {\nwhile rounds > 0 {\nif choose { let old = pair.first^ }\npair.first = 1\nchoose = !choose\nrounds -= 1\n}\n}",
    );
    let root = program.instance_program().root();
    let pair = program
        .instance_program()
        .body(root)
        .unwrap()
        .typed()
        .body()
        .inputs()
        .explicit()[0]
        .local;
    let steps = trace(&program, &[true, true, true, false, false]);
    assert_eq!(steps.iter().filter(|step| matches!(step, Step::Resource(ResourceStep::Dispose { source, payload })
        if source.holder() == Holder::Local(pair) && !source.fields().is_root() && payload.disposal() == Disposal::Trivial)).count(), 1);
    assert!(
        !steps
            .iter()
            .any(|step| matches!(step, Step::Resource(ResourceStep::ReleaseContainer { .. })))
    );
}

#[test]
fn a_direct_take_updates_flags_before_the_callee_receives_the_value() {
    let program = lower_source(
        "func consume(take value: i32): void {}\nfunc run(flag: bool): void {\nvar value = 1\nif flag { let saved = value^ }\nvalue = 2\nconsume(value^)\n}",
    );
    let id = program.instance_program().root();
    let action = program.bodies()[&id]
        .actions()
        .iter()
        .find(|(at, _)| matches!(operation(&program, id, **at), Operation::CommitCall(_)))
        .unwrap()
        .1;
    for alternative in action.alternatives() {
        assert!(
            alternative
                .flags()
                .before_writes()
                .iter()
                .any(|bit| !bit.value())
        );
        assert!(alternative.flags().after_writes().is_empty());
    }
    trace(&program, &[true]);
    trace(&program, &[false]);
}

#[test]
fn many_flag_updates_preserve_a_still_live_error_owner() {
    let mut source = String::from(
        "func run(take anchor: error, flag: bool): void {\nif flag { let saved = anchor^ }\n",
    );
    for index in 0..70 {
        writeln!(
            source,
            "var value{index} = 1\nif flag {{ let saved = value{index}^ }}\nvalue{index} = 2"
        )
        .unwrap();
    }
    source.push_str("}\n");
    let program = lower_source(&source);
    let id = program.instance_program().root();
    let anchor = program
        .instance_program()
        .body(id)
        .unwrap()
        .typed()
        .body()
        .inputs()
        .explicit()[0]
        .local;
    let retired = trace(&program, &[true; 71]);
    assert!(!retired.iter().any(|step| matches!(step, Step::Resource(ResourceStep::Dispose { source, .. }) if source.holder() == Holder::Local(anchor))));
    let retained = trace(&program, &[false; 71]);
    assert_eq!(retained.iter().filter(|step| matches!(step, Step::Resource(ResourceStep::Dispose { source, .. }) if source.holder() == Holder::Local(anchor))).count(), 1);
}

#[test]
fn equivalent_work_shares_a_guard_while_independent_cleanup_choices_remain_distinct() {
    let program = lower_source(
        "func run(take anchor: error, first: bool, second: bool): void {\nif first { let saved = anchor^ }\nvar value = 1\nif second { let saved = value^ }\nvalue = 2\n}",
    );
    let id = program.instance_program().root();
    let checked = program.instance_program().body(id).unwrap();
    let value = checked
        .typed()
        .body()
        .locals()
        .iter()
        .find(|(_, local)| {
            local.origin == nocter_checking::value_access::LocalOrigin::Binding
                && local.kind == nocter_model::ValueBindingKind::MutableValue
        })
        .unwrap()
        .0;
    let (at, assignment) = program.bodies()[&id]
        .actions()
        .iter()
        .find(|(at, _)| matches!(operation(&program, id, **at), Operation::Assign { .. }))
        .unwrap();
    assert!(checked.access().resources().at(*at).count() > assignment.alternatives().len());
    assert!(
        assignment
            .alternatives()
            .iter()
            .all(|a| a.flags().tests().len() <= 1)
    );
    assert!(program.bodies()[&id].actions().iter().any(|(&at, action)| {
        checked.access().resources().at(at).count() > 1
            && action.alternatives().len() == 1
            && action.alternatives()[0].flags().tests().is_empty()
    }));
    for first in [false, true] {
        for second in [false, true] {
            let selected = trace(&program, &[first, second]);
            let disposals = selected.iter().filter(|step| matches!(step,
                Step::Resource(ResourceStep::Dispose { source, .. }) if source.holder() == Holder::Local(value)
            )).count();
            assert_eq!(disposals, if second { 1 } else { 2 });
        }
    }
}
