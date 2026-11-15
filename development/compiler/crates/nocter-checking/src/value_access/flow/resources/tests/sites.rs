//! Payload type equality cannot choose between the old value and its replacement.
use super::*;

#[test]
fn representation_transport_keeps_source_and_destination_contract_roles() {
    let mut locals = nocter_model::ArenaBuilder::new();
    let source = Holder::Local(locals.insert(())).place();
    let destination = Holder::Return;
    let step = Step::Transport {
        source: source.clone(),
        destination,
        from: 7,
        payload: 7,
    };
    let mut sites = Vec::new();
    let mapped = step
        .try_map_payloads_at::<(), _>(|site, _| {
            sites.push(match site {
                PayloadSite::Source(place) => (Some(place.clone()), None),
                PayloadSite::Destination(holder) => (None, Some(holder)),
            });
            Ok(match site {
                PayloadSite::Source(_) => "source",
                PayloadSite::Destination(_) => "destination",
            })
        })
        .unwrap();
    assert_eq!(
        sites,
        [(Some(source.clone()), None), (None, Some(destination))]
    );
    assert_eq!(
        mapped,
        Step::Transport {
            source: source.clone(),
            destination,
            from: "source",
            payload: "destination"
        }
    );
    let mut visited = 0;
    assert_eq!(
        step.try_map_payloads_at::<_, ()>(|_, _| {
            visited += 1;
            Err("source unavailable")
        }),
        Err("source unavailable")
    );
    assert_eq!(visited, 1);
}

#[test]
fn scoped_replacement_maps_the_retired_destination_before_the_supplied_source() {
    let mut locals = nocter_model::ArenaBuilder::new();
    let destination = Holder::Local(locals.insert(())).place();
    let source = Holder::Local(locals.insert(())).place();
    let step = Step::ReplaceReferent {
        destination: destination.clone(),
        source: source.clone(),
        retired: 7,
        payload: 7,
    };
    let mut sites = Vec::new();
    let projected = step
        .try_map_payloads_at::<(), _>(|site, _| {
            let PayloadSite::Source(place) = site else {
                panic!("an existing value")
            };
            sites.push(place.clone());
            Ok(if place == &destination {
                "retired"
            } else if place == &source {
                "supplied"
            } else {
                panic!("original place")
            })
        })
        .unwrap();
    assert_eq!(sites, [destination.clone(), source.clone()]);
    assert_eq!(
        projected,
        Step::ReplaceReferent {
            destination,
            source,
            retired: "retired",
            payload: "supplied"
        }
    );
    let mut visited = 0;
    assert_eq!(
        step.try_map_payloads_at::<_, ()>(|_, _| {
            visited += 1;
            Err("retired payload unavailable")
        }),
        Err("retired payload unavailable")
    );
    assert_eq!(
        visited, 1,
        "a failed old-value projection cannot publish a replacement"
    );
}

#[test]
fn replacement_cleanup_and_transfer_select_different_actual_values_with_the_same_source_type() {
    let queries = queries("func first(): i32 { 41 }\nfunc second(): i32 { 42 }\nfunc work(take action: func(): i32): void {}").unwrap();
    let program = queries[0].program();
    let code = |name| {
        program
            .resolve(program.symbols().get(name).unwrap())
            .unwrap()
    };
    let work = code("work");
    let typed = queries
        .iter()
        .find(|query| query.code() == nocter_model::ValueCode::Declaration(work))
        .unwrap();
    let input = typed.body().inputs().explicit()[0];
    let data = typed.body().locals().get(input.local).unwrap().data;
    let parameter = program.codes().get(work).unwrap().parameters()[0];
    let keys = ["first", "second"].map(|name| {
        let frame = program
            .bind_code(work.into(), [(parameter.into(), code(name).into())])
            .unwrap();
        program.instantiate_data(data, &frame).unwrap()
    });
    assert_eq!(keys[0].data(), keys[1].data());
    assert_ne!(keys[0], keys[1]);
    let owned = payload(program, data).unwrap();
    let old = Holder::Local(input.local).place();
    let supplied = Holder::Return.place();
    let steps = [
        Step::Dispose {
            source: old.clone(),
            payload: owned,
        },
        Step::Move {
            source: supplied.clone(),
            destination: old.clone(),
            payload: owned,
        },
    ];
    let projected = steps
        .iter()
        .map(|step| {
            step.try_map_payloads_at(|site, contract| {
                let key = match site {
                    PayloadSite::Source(place) if place == &old => &keys[0],
                    PayloadSite::Source(place) if place == &supplied => &keys[1],
                    _ => return Err(INVALID),
                };
                contract.instantiate(program, key)
            })
        })
        .collect::<Result<Vec<_>>>()
        .unwrap();
    let [
        Step::Dispose {
            payload: old_value, ..
        },
        Step::Move {
            payload: new_value, ..
        },
    ] = projected.as_slice()
    else {
        panic!("ordered cleanup then transfer")
    };
    assert_ne!(old_value.instance(), new_value.instance());
    assert_eq!(
        old_value.instance(),
        &keys[0].function_representation(program).unwrap().unwrap()
    );
    assert_eq!(
        new_value.instance(),
        &keys[1].function_representation(program).unwrap().unwrap()
    );
}

#[test]
fn capture_mapping_keeps_environment_source_separate_from_the_new_callable_destination() {
    let mut locals = nocter_model::ArenaBuilder::new();
    let source = Holder::Local(locals.insert(())).place();
    let destination = Holder::Return;
    let step = Step::Capture {
        source: source.clone(),
        destination,
        environment: 7,
        payload: 7,
    };
    let projected = step
        .try_map_payloads_at::<(), _>(|site, _| {
            Ok(match site {
                PayloadSite::Source(place) if place == &source => "environment",
                PayloadSite::Destination(holder) if holder == destination => "callable",
                _ => panic!("unexpected payload source"),
            })
        })
        .unwrap();
    assert_eq!(
        projected,
        Step::Capture {
            source: source.clone(),
            destination,
            environment: "environment",
            payload: "callable"
        }
    );
    let mut visited = 0;
    assert_eq!(
        step.try_map_payloads_at::<_, ()>(|_, _| {
            visited += 1;
            if visited == 1 { Ok(()) } else { Err("stop") }
        }),
        Err("stop")
    );
    assert_eq!(
        step.try_map_payloads::<(), _>(|value| Ok(*value)).unwrap(),
        step
    );
}
