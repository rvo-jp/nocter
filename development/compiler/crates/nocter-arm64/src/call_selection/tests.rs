//! Incoming payloads do not inherit a receiving slot's capacity for later representations.
use super::*;

#[test]
fn incoming_abi_sizes_survive_larger_receiving_capacity_and_preserve_address_carriers() {
    let machine = crate::test_support::lower_value_machine(
        "struct Wide { a: i32\nb: i32\nc: i32\nd: i32\ne: i32 }\n\
         func forward(take a: i32, take b: i32, take c: i32, take d: i32, take e: i32, take f: i32, take g: i32, take h: i32, take i: i32, view: Wide, take owned: Wide, take empty: void): i32 {\n\
         a + b + c + d + e + f + g + h + i + view.e + owned.e }\n\
         func run(): i32 { let wide = Wide { a: 1, b: 2, c: 3, d: 4, e: 12 }\n\
         let owned = Wide { a: 1, b: 2, c: 3, d: 4, e: 21 }\n\
         forward(1, 1, 1, 1, 1, 1, 1, 1, 1, wide, owned^, ()) }",
    );
    let (owner, function, abi) = machine
        .functions()
        .find_map(|(owner, function)| {
            let MachineFunctionKind::Callable(abi) = function.kind() else {
                return None;
            };
            (abi.arguments().len() == 12).then_some((owner, function, abi))
        })
        .expect("the source forward function");
    let expected = [4, 4, 4, 4, 4, 4, 4, 4, 4, 8, 20, 0];
    for ((&stack, argument), size) in function
        .body()
        .parameters()
        .iter()
        .zip(abi.arguments())
        .zip(expected)
    {
        let object = function.body().stack(stack).unwrap();
        assert_eq!(
            parameter_size(&machine, function, stack, argument),
            Ok(size)
        );
        assert_eq!(
            incoming_size(machine.layouts(), argument, object.carrier(), 128),
            Some(size)
        );
        if size > 0 {
            assert_eq!(
                incoming_size(machine.layouts(), argument, object.carrier(), size - 1),
                None
            );
        }
    }
    assert!(
        abi.arguments()
            .iter()
            .any(|argument| matches!(argument.location(), Some(MachineArgumentLocation::Stack(_))))
    );
    let selected = crate::Arm64SelectedFunction::build(&machine, owner).unwrap();
    let copied = selected
        .entry_instructions()
        .iter()
        .filter_map(|instruction| {
            if let Arm64SelectedInstruction::CopyMemoryNonOverlapping { bytes, .. } = instruction {
                Some(*bytes)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(copied, [20]);
}

#[test]
fn growing_owned_environments_copy_only_the_original_direct_or_indirect_input() {
    let machine = crate::test_support::lower_value_machine(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/native/value-access/contextual-owning-scoped-growth.nct"
    )));
    let mut direct = 0;
    let mut indirect = 0;
    for (owner, function) in machine.functions() {
        let MachineFunctionKind::Callable(abi) = function.kind() else {
            continue;
        };
        for (&stack, argument) in function.body().parameters().iter().zip(abi.arguments()) {
            let object = function.body().stack(stack).unwrap();
            let incoming = machine.layouts().get(argument.ty()).unwrap().size();
            if object.carrier() != nocter_machine::MachineValueCarrier::Data
                || object.size() <= incoming
            {
                continue;
            }
            let selected = crate::Arm64SelectedFunction::build(&machine, owner).unwrap();
            let target = selected.frame().stack_object(stack).unwrap();
            let destination = |address: &Arm64SelectedMemoryAddress| {
                matches!(address,
                    Arm64SelectedMemoryAddress::Stack(Arm64SelectedStackAddress::FrameObject { object, .. }) if *object == target
                )
            };
            match argument.class() {
                MachineValueClass::Direct { words: 1 } => {
                    assert_eq!((incoming, object.size()), (4, 36));
                    let stored = selected
                        .entry_instructions()
                        .iter()
                        .filter_map(|instruction| match instruction {
                            Arm64SelectedInstruction::StoreMemory {
                                bytes,
                                destination: address,
                                ..
                            } if destination(address) => Some(*bytes),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(stored, [4]);
                    direct += 1;
                }
                MachineValueClass::Indirect => {
                    assert_eq!((incoming, object.size()), (36, 68));
                    let copied = selected
                        .entry_instructions()
                        .iter()
                        .filter_map(|instruction| match instruction {
                            Arm64SelectedInstruction::CopyMemoryNonOverlapping {
                                bytes,
                                destination: address,
                                ..
                            } if destination(address) => Some(*bytes),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(copied, [36]);
                    indirect += 1;
                }
                class => panic!("unexpected growing input: {class:?}"),
            }
        }
    }
    assert_eq!((direct, indirect), (1, 1));
}
