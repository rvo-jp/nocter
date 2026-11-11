use super::*;

#[test]
fn current_representative_updates_leave_old_snapshots_intact() {
    let mut state = Equalities::top(3).unwrap();
    assert!(state.assign(2, 1, 0));
    assert!(state.assign(3, 1, 7));
    assert!(state.assign(1, 1, 1));
    assert_eq!(state.difference(1, 2), Some(1));
    assert_eq!(state.difference(3, 2), Some(7));
    assert!(state.forget(1));
    assert_eq!(state.difference(1, 2), None);
    assert_eq!(state.difference(3, 2), Some(7));
    assert!(state.assign(1, 3, -2));
    assert_eq!(state.difference(1, 2), Some(5));
    assert!(state.assign(2, 1, 0));
    assert_eq!(state.difference(3, 2), Some(2));
}

#[test]
fn maximum_values_contradictions_and_bottom_join_are_exact() {
    let mut full = Equalities::top(2).unwrap();
    assert!(full.assign(1, ZERO, LIMIT));
    assert!(full.assign(2, 1, -LIMIT));
    assert_eq!(full.constant(1), Some(u64::MAX));
    assert_eq!(full.constant(2), Some(0));
    let mut overflow = full.clone();
    assert!(overflow.assign(1, 1, 1));
    assert!(!overflow.possible());
    assert_eq!(overflow.difference(1, 1), None);
    assert_eq!(overflow.join(&full), Some(full.clone()));
    assert_eq!(full.join(&overflow), Some(full.clone()));
    assert!(full.assume(1, 2, 0));
    assert!(!full.possible());
    let mut unanchored = Equalities::top(3).unwrap();
    assert!(unanchored.assume(1, 2, LIMIT));
    assert!(unanchored.assume(2, 3, 1));
    assert!(!unanchored.possible());
    let mut another_bottom = Equalities::top(3).unwrap();
    another_bottom.assign(1, ZERO, -1);
    assert_eq!(unanchored, another_bottom);
}

#[test]
fn invalid_cells_and_dimensions_do_not_silently_supply_a_proof() {
    assert!(Equalities::top(usize::MAX).is_none());
    let mut state = Equalities::top(2).unwrap();
    assert!(!state.assign(ZERO, 1, 0));
    assert!(!state.assign(3, 1, 0));
    assert!(!state.forget(ZERO));
    assert!(!state.assume(1, 3, 0));
    assert!(state.join(&Equalities::top(3).unwrap()).is_none());
    assert!(state.assign(1, ZERO, 9));
    assert_eq!(state.constant(1), Some(9));
}

#[test]
fn canonical_join_keeps_common_offsets_and_reaches_a_fixed_point_without_a_cutoff() {
    let mut first = Equalities::top(3).unwrap();
    first.assign(1, ZERO, 0);
    first.assign(2, 1, 1);
    first.assign(3, 1, 4);
    let mut next = first.clone();
    next.assign(1, 1, 1);
    next.assign(2, 2, 1);
    next.assign(3, 3, 1);
    let joined = first.join(&next).unwrap();
    assert_eq!(joined.constant(1), None);
    assert_eq!(joined.difference(2, 1), Some(1));
    assert_eq!(joined.difference(3, 1), Some(4));
    let mut following = joined.clone();
    for cell in 1..=3 {
        following.assign(cell, cell, 1);
    }
    assert_eq!(joined.join(&following), Some(joined.clone()));
    assert_eq!(first.join(&next), next.join(&first));
    assert_eq!(joined.join(&joined), Some(joined));
}

fn check(state: &Equalities, values: &[u64]) {
    assert!(
        state.possible(),
        "nonempty concrete witnesses must not become bottom: {values:?}"
    );
    for left in 0..values.len() {
        for right in 0..values.len() {
            if let Some(difference) = state.difference(left, right) {
                assert_eq!(
                    difference,
                    i128::from(values[left]) - i128::from(values[right]),
                    "{values:?}"
                );
            }
        }
    }
}
#[test]
fn independent_concrete_witnesses_validate_assignment_forgetting_and_common_join() {
    // Concrete usize tuples are the oracle, rather than another weighted-partition algorithm.
    // Include extreme values: small-domain exhaustive tests alone miss unsigned boundaries.
    let numbers = [0_u64, 1, 2, 7, u64::MAX - 1, u64::MAX];
    for &a in &numbers {
        for &b in &numbers {
            for &c in &numbers {
                let original = [0, a, b, c];
                let mut state = Equalities::top(3).unwrap();
                for (cell, &number) in original.iter().enumerate().skip(1) {
                    state.assign(cell, ZERO, i128::from(number));
                }
                check(&state, &original);
                for target in 1..=3 {
                    for source in 0..=3 {
                        for offset in [-2, -1, 0, 1, 2] {
                            let value = i128::from(original[source]) + offset;
                            let mut assigned = state.clone();
                            assigned.assign(target, source, offset);
                            if let Ok(value) = u64::try_from(value) {
                                let mut concrete = original;
                                concrete[target] = value;
                                check(&assigned, &concrete);
                                let joined = state.join(&assigned).unwrap();
                                check(&joined, &original);
                                check(&joined, &concrete);
                                assert_eq!(state.join(&assigned), assigned.join(&state));
                            } else {
                                assert!(!assigned.possible());
                            }
                        }
                    }
                }
                for target in 1..=3 {
                    let mut forgotten = state.clone();
                    forgotten.forget(target);
                    for &value in &numbers {
                        let mut concrete = original;
                        concrete[target] = value;
                        check(&forgotten, &concrete);
                    }
                }
            }
        }
    }
}

#[test]
fn unanchored_classes_preserve_every_concrete_witness_through_updates_and_refinement() {
    let numbers = [0_u64, 1, 2, u64::MAX - 1, u64::MAX];
    for &a in &numbers {
        for &b in &numbers {
            for &c in &numbers {
                let original = [0, a, b, c];
                for mask in 0..8 {
                    let mut state = Equalities::top(3).unwrap();
                    for (bit, (left, right)) in [(1, 2), (2, 3), (1, 3)].into_iter().enumerate() {
                        if mask & (1 << bit) != 0 {
                            state.assume(
                                left,
                                right,
                                i128::from(original[left]) - i128::from(original[right]),
                            );
                        }
                    }
                    check(&state, &original);
                    for target in 1..=3 {
                        for source in 0..=3 {
                            for offset in [-1, 0, 1] {
                                let mut assigned = state.clone();
                                assigned.assign(target, source, offset);
                                if let Ok(value) =
                                    u64::try_from(i128::from(original[source]) + offset)
                                {
                                    let mut concrete = original;
                                    concrete[target] = value;
                                    check(&assigned, &concrete);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
