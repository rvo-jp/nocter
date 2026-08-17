use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_mir::lower_executable;
use nocter_model::{BorrowCapability, BuiltinType, CompilationTarget, TypeId, TypeKind};
use nocter_target_program::{
    ExecutableProgram, PrimitiveBinding, PrimitiveRegistry, PrimitiveRole, TargetProgram,
    ToolchainSnapshot,
};
use nocter_test_support::CompilerFixture;

use crate::{
    MachineAbiPlan, MachineArgumentLocation, MachineDataTable, MachineEndianness,
    MachineEnumVariantLayout, MachineLayoutKind, MachineLayoutStore, MachineLinkageKey,
    MachineLinkageTable, MachineOperationKind, MachineOutcomeKind, MachineProgram,
    MachineProgramRoot, MachineResultAbi, MachineResultLocation, MachineRootLinkage, MachineScalar,
    MachineTerminator, MachineValueClass,
};

#[test]
fn computes_the_complete_arm64_stored_layout_closure() {
    let program = stored_layout_fixture();
    let layouts = MachineLayoutStore::build(&program).unwrap();

    assert_eq!(layouts.target().word_size(), 8);
    assert_eq!(layouts.target().stack_alignment(), 16);
    assert_eq!(layouts.target().endianness(), MachineEndianness::Little);
    assert_aggregate_layouts(&program, &layouts);
    assert_scalar_view_and_outcome_layouts(&program, &layouts);
}

fn stored_layout_fixture() -> nocter_mir::MirProgram {
    lower_fixture(
        "struct Empty {}\n\
         struct Pair {\n\
             small: u8\n\
             wide: u64\n\
         }\n\
         enum Choice {\n\
             empty\n\
             one(value: u32)\n\
             pair(first: u8, second: u64)\n\
         }\n\
         func main(): i32! {\n\
             let empty = Empty {}\n\
             let pair = Pair { small: 1, wide: 2 }\n\
             let choice = Choice.pair(3, 4)\n\
             let values: [u16; 3] = [1, 2, 3]\n\
             let markers: [Empty; 3] = [Empty {}, Empty {}, Empty {}]\n\
             let text: &str = \"ok\"\n\
             let number: i32 = 5\n\
             let number_ref = &number\n\
             complete()?\n\
             return 0\n\
         }\n\
         func complete(): void! { return }\n",
    )
}

fn assert_aggregate_layouts(program: &nocter_mir::MirProgram, layouts: &MachineLayoutStore) {
    let types = program.executable().types();
    let empty = named_nominal(program, "Empty");
    let empty_layout = layouts.get(empty).unwrap();
    assert_eq!((empty_layout.size(), empty_layout.alignment()), (0, 1));
    assert!(matches!(
        empty_layout.kind(),
        MachineLayoutKind::Struct { fields } if fields.is_empty()
    ));

    let pair = named_nominal(program, "Pair");
    let pair_layout = layouts.get(pair).unwrap();
    assert_eq!((pair_layout.size(), pair_layout.alignment()), (16, 8));
    let MachineLayoutKind::Struct { fields } = pair_layout.kind() else {
        panic!("Pair must use struct layout")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.offset())
            .collect::<Vec<_>>(),
        [0, 8]
    );

    let choice = named_nominal(program, "Choice");
    let choice_layout = layouts.get(choice).unwrap();
    assert_eq!((choice_layout.size(), choice_layout.alignment()), (24, 8));
    let MachineLayoutKind::Enum {
        tag_offset,
        payload_offset,
        variants,
    } = choice_layout.kind()
    else {
        panic!("Choice must use enum layout")
    };
    assert_eq!(*tag_offset, 0);
    assert_eq!(*payload_offset, 8);
    assert_eq!(variants.len(), 3);
    assert_eq!(
        variants
            .iter()
            .map(MachineEnumVariantLayout::tag)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert!(variants[0].payload().is_empty());
    assert_eq!(variants[1].payload()[0].offset(), 8);
    assert_eq!(
        variants[2]
            .payload()
            .iter()
            .map(|payload| payload.offset())
            .collect::<Vec<_>>(),
        [8, 16]
    );

    let array = types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(
                kind,
                TypeKind::FixedArray { element, length }
                    if *element == types.builtin(BuiltinType::U16) && *length == 3
            )
            .then_some(ty)
        })
        .unwrap();
    let array_layout = layouts.get(array).unwrap();
    assert_eq!((array_layout.size(), array_layout.alignment()), (6, 2));
    assert!(matches!(
        array_layout.kind(),
        MachineLayoutKind::FixedArray { stride: 2, .. }
    ));

    let empty_array = types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(
                kind,
                TypeKind::FixedArray { element, length }
                    if *element == empty && *length == 3
            )
            .then_some(ty)
        })
        .unwrap();
    let empty_array_layout = layouts.get(empty_array).unwrap();
    assert_eq!(
        (empty_array_layout.size(), empty_array_layout.alignment()),
        (0, 1)
    );
    assert!(matches!(
        empty_array_layout.kind(),
        MachineLayoutKind::FixedArray { stride: 0, .. }
    ));
}

fn assert_scalar_view_and_outcome_layouts(
    program: &nocter_mir::MirProgram,
    layouts: &MachineLayoutStore,
) {
    let types = program.executable().types();
    let str_borrow = borrow_type(types, BuiltinType::Str);
    let i32_borrow = borrow_type(types, BuiltinType::I32);
    assert!(matches!(
        layouts.get(str_borrow).unwrap().kind(),
        MachineLayoutKind::View {
            pointer_offset: 0,
            length_offset: 8,
        }
    ));
    assert_eq!(layouts.get(str_borrow).unwrap().size(), 16);
    assert!(matches!(
        layouts.get(i32_borrow).unwrap().kind(),
        MachineLayoutKind::Pointer
    ));
    assert_eq!(layouts.get(i32_borrow).unwrap().size(), 8);

    let fallible_i32 = types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(kind, TypeKind::Fallible(payload) if *payload == types.builtin(BuiltinType::I32))
                .then_some(ty)
        })
        .unwrap();
    let outcome = layouts.get(fallible_i32).unwrap();
    assert_eq!((outcome.size(), outcome.alignment()), (40, 8));
    assert!(matches!(
        outcome.kind(),
        MachineLayoutKind::Outcome {
            kind: MachineOutcomeKind::Fallible,
            tag_offset: 0,
            payload_offset: 8,
            primary: Some(primary),
            alternate: Some(alternate),
        } if *primary == types.builtin(BuiltinType::I32)
            && *alternate == types.builtin(BuiltinType::Error)
    ));
    let error = layouts.get(types.builtin(BuiltinType::Error)).unwrap();
    assert_eq!((error.size(), error.alignment()), (32, 8));
    assert!(matches!(
        error.kind(),
        MachineLayoutKind::Error {
            code_offset: 0,
            message_offset: 16,
        }
    ));
    assert!(matches!(
        layouts.get(types.builtin(BuiltinType::I32)).unwrap().kind(),
        MachineLayoutKind::Scalar(MachineScalar::Integer {
            bits: 32,
            signed: true,
        })
    ));

    let fallible_void = types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(kind, TypeKind::Fallible(payload) if *payload == types.builtin(BuiltinType::Void))
                .then_some(ty)
        })
        .unwrap();
    let completion = layouts.get(fallible_void).unwrap();
    assert_eq!((completion.size(), completion.alignment()), (40, 8));
    assert!(matches!(
        completion.kind(),
        MachineLayoutKind::Outcome {
            kind: MachineOutcomeKind::Fallible,
            tag_offset: 0,
            payload_offset: 8,
            primary: None,
            alternate: Some(alternate),
        } if *alternate == types.builtin(BuiltinType::Error)
    ));
}

#[test]
fn closure_layout_uses_the_executable_capture_order_and_concrete_types() {
    let program = lower_fixture(
        "struct Small { value: u8 }\n\
         struct Wide { value: u64 }\n\
         func main(): void {\n\
             let small = Small { value: 1 }\n\
             let wide = Wide { value: 2 }\n\
             let callback = (move small, move wide;): void { return }\n\
             callback()\n\
             return\n\
         }\n",
    );
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let closure = program
        .executable()
        .types()
        .iter()
        .find_map(|(ty, kind)| matches!(kind, TypeKind::Closure { .. }).then_some(ty))
        .unwrap();
    let layout = layouts.get(closure).unwrap();

    assert_eq!((layout.size(), layout.alignment()), (16, 8));
    let MachineLayoutKind::Closure { captures } = layout.kind() else {
        panic!("closure must use its concrete environment layout")
    };
    assert_eq!(
        captures
            .iter()
            .map(|capture| capture.offset())
            .collect::<Vec<_>>(),
        [0, 8]
    );
}

#[test]
fn payloadless_enum_and_optional_keep_their_distinct_tagged_layouts() {
    let program = lower_fixture(
        "enum Flag {\n\
             off\n\
             on\n\
         }\n\
         func main(): void {\n\
             let flag = Flag.off\n\
             let maybe: u16? = none\n\
             return\n\
         }\n",
    );
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let flag = named_nominal(&program, "Flag");
    let flag_layout = layouts.get(flag).unwrap();
    assert_eq!((flag_layout.size(), flag_layout.alignment()), (1, 1));
    assert!(matches!(
        flag_layout.kind(),
        MachineLayoutKind::Enum {
            tag_offset: 0,
            payload_offset: 1,
            variants,
        } if variants.len() == 2 && variants.iter().all(|variant| variant.payload().is_empty())
    ));

    let types = program.executable().types();
    let optional = types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(kind, TypeKind::Optional(payload) if *payload == types.builtin(BuiltinType::U16))
                .then_some(ty)
        })
        .unwrap();
    let optional_layout = layouts.get(optional).unwrap();
    assert_eq!(
        (optional_layout.size(), optional_layout.alignment()),
        (4, 2)
    );
    assert!(matches!(
        optional_layout.kind(),
        MachineLayoutKind::Outcome {
            kind: MachineOutcomeKind::Optional,
            tag_offset: 0,
            payload_offset: 2,
            primary: Some(primary),
            alternate: None,
        } if *primary == types.builtin(BuiltinType::U16)
    ));
}

#[test]
fn opaque_layout_is_exactly_its_specialized_witness_layout() {
    let program = lower_fixture(
        "pub interface Show {\n\
             pub method &self.show(): i32\n\
         }\n\
         struct Value {\n\
             marker: u8\n\
             value: u64\n\
         }\n\
         conform Show for Value {\n\
             method &self.show(): i32 { 7 }\n\
         }\n\
         func make(): some Show { Value { marker: 1, value: 2 } }\n\
         func main(): i32 { make().show() }\n",
    );
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let types = program.executable().types();
    let opaque = types
        .iter()
        .find_map(|(ty, kind)| matches!(kind, TypeKind::Opaque { .. }).then_some(ty))
        .unwrap();
    let layout = layouts.get(opaque).unwrap();
    let MachineLayoutKind::Opaque { witness } = layout.kind() else {
        panic!("opaque type must retain its concrete representation witness")
    };
    let witness = layouts.get(*witness).unwrap();

    assert_eq!(
        (layout.size(), layout.alignment()),
        (witness.size(), witness.alignment())
    );
    assert_eq!((layout.size(), layout.alignment()), (16, 8));
}

#[test]
fn abi_argument_spill_closes_the_register_window_without_reusing_x7() {
    let program = abi_fixture();
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let abi = MachineAbiPlan::build(&program, &layouts).unwrap();
    let placement = abi
        .iter()
        .map(|(_, callable)| callable)
        .find(|callable| callable.arguments().len() == 10)
        .unwrap();

    for (register, argument) in placement.arguments()[..7].iter().enumerate() {
        assert!(matches!(
            argument.location(),
            Some(MachineArgumentLocation::Registers(span))
                if usize::from(span.first()) == register && span.words() == 1
        ));
    }
    assert!(matches!(
        placement.arguments()[7].location(),
        Some(MachineArgumentLocation::Stack(slot))
            if slot.offset() == 0 && slot.size() == 16 && slot.alignment() == 8
    ));
    assert_eq!(placement.arguments()[8].class(), MachineValueClass::Zero);
    assert_eq!(placement.arguments()[8].location(), None);
    assert!(matches!(
        placement.arguments()[9].location(),
        Some(MachineArgumentLocation::Stack(slot))
            if slot.offset() == 16 && slot.size() == 8 && slot.alignment() == 8
    ));
    assert_eq!(placement.stack_argument_size(), 32);
}

#[test]
fn abi_classifies_indirect_arguments_and_all_return_forms_from_stored_layouts() {
    let program = abi_fixture();
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let abi = MachineAbiPlan::build(&program, &layouts).unwrap();
    let empty = named_nominal(&program, "Empty");
    let pair = named_nominal(&program, "Pair");
    let large = named_nominal(&program, "Large");

    let indirect = abi
        .iter()
        .map(|(_, callable)| callable)
        .find(|callable| {
            callable.arguments().len() == 2
                && callable.arguments()[0].class() == MachineValueClass::Indirect
        })
        .unwrap();
    assert!(matches!(
        indirect.arguments()[0].location(),
        Some(MachineArgumentLocation::Registers(span))
            if span.first() == 0 && span.words() == 1
    ));
    assert!(matches!(
        indirect.arguments()[1].location(),
        Some(MachineArgumentLocation::Registers(span))
            if span.first() == 1 && span.words() == 1
    ));

    assert_return(&abi, empty, MachineValueClass::Zero, |location| {
        location == MachineResultLocation::Omitted
    });
    assert_return(
        &abi,
        pair,
        MachineValueClass::Direct { words: 2 },
        |location| {
            matches!(
                location,
                MachineResultLocation::Registers(span) if span.first() == 0 && span.words() == 2
            )
        },
    );
    assert_return(&abi, large, MachineValueClass::Indirect, |location| {
        matches!(
            location,
            MachineResultLocation::CallerStorage {
                pointer_register: 8
            }
        )
    });
    assert!(
        abi.iter()
            .any(|(_, callable)| callable.result() == MachineResultAbi::Diverging)
    );
    assert!(
        abi.iter()
            .any(|(_, callable)| callable.result() == MachineResultAbi::Completion)
    );
}

fn assert_return(
    abi: &MachineAbiPlan,
    ty: TypeId,
    class: MachineValueClass,
    location_matches: impl Fn(MachineResultLocation) -> bool,
) {
    assert!(abi.iter().any(|(_, callable)| {
        matches!(
            callable.result(),
            MachineResultAbi::Value(value)
                if value.ty() == ty && value.class() == class && location_matches(value.location())
        )
    }));
}

fn abi_fixture() -> nocter_mir::MirProgram {
    lower_fixture(
        "struct Empty {}\n\
         struct Pair {\n\
             small: u8\n\
             wide: u64\n\
         }\n\
         struct Large {\n\
             first: u64\n\
             second: u64\n\
             third: u64\n\
         }\n\
         func place(\n\
             a0: u64,\n\
             a1: u64,\n\
             a2: u64,\n\
             a3: u64,\n\
             a4: u64,\n\
             a5: u64,\n\
             a6: u64,\n\
             pair: &str,\n\
             marker: Empty,\n\
             tail: u64,\n\
         ): void { return }\n\
         func accept_large(value: Large, tail: u64): void { return }\n\
         func make_empty(): Empty { Empty {} }\n\
         func make_pair(): Pair { Pair { small: 1, wide: 2 } }\n\
         func make_large(): Large { Large { first: 1, second: 2, third: 3 } }\n\
         func halt(): never { loop {} }\n\
         func main(): void {\n\
             place(0, 1, 2, 3, 4, 5, 6, \"two words\", Empty {}, 9)\n\
             let argument = Large { first: 1, second: 2, third: 3 }\n\
             accept_large(move argument, 10)\n\
             let empty = make_empty()\n\
             let pair = make_pair()\n\
             let large = make_large()\n\
             drop empty\n\
             drop pair\n\
             drop large\n\
             if false { halt() }\n\
             return\n\
         }\n",
    )
}

#[test]
fn literal_pack_uses_one_compiler_owned_pointer_lane_outside_ordinary_arguments() {
    let program = lower_fixture(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): void {\n\
             let values = Vec [1, 2]\n\
             drop values\n\
             return\n\
         }\n",
    );
    let layouts = MachineLayoutStore::build(&program).unwrap();
    let abi = MachineAbiPlan::build(&program, &layouts).unwrap();
    let literal = abi
        .iter()
        .map(|(_, callable)| callable)
        .find(|callable| callable.pack().is_some())
        .unwrap();
    let pack = literal.pack().unwrap();

    assert!(literal.arguments().is_empty());
    assert_eq!(pack.pointer().first(), 0);
    assert_eq!(pack.pointer().words(), 1);
    assert_eq!(literal.stack_argument_size(), 0);
}

#[test]
fn linkage_uses_semantic_owners_and_static_text_uses_sorted_byte_identity() {
    let program = lower_fixture(
        "func main(): void {\n\
             let last: &str = \"z\"\n\
             let first: &str = \"a\"\n\
             let repeated: &str = \"z\"\n\
             return\n\
         }\n",
    );
    let data = MachineDataTable::build(&program);
    assert_eq!(data.len(), 2);
    assert_eq!(
        data.iter()
            .map(|(_, entry)| entry.bytes())
            .collect::<Vec<_>>(),
        [b"a".as_slice(), b"z".as_slice()]
    );
    assert!(data.text("a").is_some());
    assert!(data.text("z").is_some());
    assert_ne!(data.text("a"), data.text("z"));

    let linkage = MachineLinkageTable::build(&program).unwrap();
    let item_count = linkage
        .iter()
        .filter(|(_, entry)| matches!(entry.key(), MachineLinkageKey::Item(_)))
        .count();
    assert_eq!(item_count, program.functions().len());
    let nocter_mir::MirRoot::Process(root) = program.root() else {
        panic!("fixture must have one process root")
    };
    let MachineRootLinkage::Process {
        target,
        process,
        entry,
    } = linkage.root()
    else {
        panic!("linkage must retain the process root")
    };
    assert_eq!(*target, root.target());
    assert_eq!(
        Some(*process),
        linkage.id(MachineLinkageKey::ProcessRoot(root.target()))
    );
    assert_eq!(
        Some(*entry),
        linkage.id(MachineLinkageKey::Item(root.entry()))
    );
}

#[test]
fn test_root_linkage_retains_declaration_order_separately_from_key_order() {
    let program = lower_test_fixture(
        "test first { return }\n\
         test second { return }\n",
    );
    let linkage = MachineLinkageTable::build(&program).unwrap();
    let nocter_mir::MirRoot::Tests {
        target,
        cases: mir_cases,
    } = program.root()
    else {
        panic!("fixture must have test roots")
    };
    let MachineRootLinkage::Tests {
        target: linked_target,
        cases,
    } = linkage.root()
    else {
        panic!("linkage must retain test roots")
    };
    assert_eq!(linked_target, target);
    assert_eq!(cases.len(), 2);
    for (linked, source) in cases.iter().zip(mir_cases) {
        assert_eq!(linked.declaration(), source.declaration());
        assert_eq!(linked.name(), source.name());
        assert_eq!(
            Some(linked.test()),
            linkage.id(MachineLinkageKey::TestRoot(source.declaration()))
        );
        assert_eq!(
            Some(linked.body()),
            linkage.id(MachineLinkageKey::Item(source.item()))
        );
    }
}

#[test]
fn machine_program_owns_dense_functions_values_operations_and_control_flow() {
    let mir = lower_fixture(
        "func main(): i32 {\n\
             if true {\n\
                 return 7\n\
             }\n\
             return 9\n\
         }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let MachineProgramRoot::Process { root, entry } = *program.root() else {
        panic!("fixture must produce one process machine root")
    };
    assert_eq!(program.functions().len(), 2);
    assert!(matches!(
        program.function(root).unwrap().kind(),
        crate::MachineFunctionKind::ProcessRoot
    ));
    assert!(matches!(
        program.function(entry).unwrap().kind(),
        crate::MachineFunctionKind::Callable(_)
    ));

    let root_body = program.function(root).unwrap().body();
    let direct_target = root_body
        .operations()
        .find_map(|(_, operation)| match operation.kind() {
            MachineOperationKind::Call(call) => match call.target() {
                crate::MachineCallTarget::Direct(target) => Some(*target),
                crate::MachineCallTarget::Primitive(_) => None,
            },
            _ => None,
        });
    assert_eq!(direct_target, Some(entry));
    assert!(
        root_body
            .blocks()
            .any(|(_, block)| matches!(block.terminator(), MachineTerminator::Exit(Some(_))))
    );

    let entry_body = program.function(entry).unwrap().body();
    assert_eq!(entry_body.values().len(), 3);
    assert_eq!(
        entry_body
            .operations()
            .filter(|(_, operation)| {
                matches!(operation.kind(), MachineOperationKind::Constant(_))
            })
            .count(),
        3
    );
    assert!(
        entry_body
            .blocks()
            .any(|(_, block)| matches!(block.terminator(), MachineTerminator::Branch { .. }))
    );
    for (_, value) in entry_body.values() {
        assert!(program.layouts().get(value.ty()).is_some());
    }
}

#[test]
fn machine_program_erases_local_places_into_stack_addresses_and_memory_operations() {
    let mir = lower_fixture(
        "func main(): i32 {\n\
             let answer: i32 = 42\n\
             return answer\n\
         }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce one process machine root")
    };
    let body = program.function(entry).unwrap().body();
    assert_eq!(body.stack_objects().len(), 1);
    let (address_id, address) = body.addresses().next().unwrap();
    assert!(matches!(
        address.root(),
        crate::MachineAddressRoot::Stack(_)
    ));
    assert!(address.steps().is_empty());
    assert!(body.operations().any(|(_, operation)| {
        matches!(
            operation.kind(),
            MachineOperationKind::Store { destination, .. } if *destination == address_id
        )
    }));
    assert!(body.operations().any(|(_, operation)| {
        matches!(
            operation.kind(),
            MachineOperationKind::Load { source } if *source == address_id
        )
    }));
}

#[test]
fn aggregate_and_field_projection_share_layout_owned_offsets() {
    let mir = lower_fixture(
        "copy struct Pair { first: u8\n    answer: i32 }\n\
         func main(): i32 {\n\
             let pair = Pair { first: 1, answer: 42 }\n\
             return pair.answer\n\
         }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce one process machine root")
    };
    let body = program.function(entry).unwrap().body();
    assert!(
        body.addresses()
            .any(|(_, address)| { address.steps() == [crate::MachineAddressStep::Offset(4)] })
    );
    assert!(body.operations().any(|(_, operation)| {
        let MachineOperationKind::Aggregate(aggregate) = operation.kind() else {
            return false;
        };
        aggregate
            .writes()
            .iter()
            .any(|write| matches!(write, crate::MachineAggregateWrite::Value { offset: 4, .. }))
    }));
}

#[test]
fn fixed_array_index_retains_stride_and_runtime_bound() {
    let mir = lower_fixture(
        "func main(): i32 {\n\
             let values: [i32; 2] = [7, 9]\n\
             return values[1]\n\
         }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let MachineProgramRoot::Process { entry, .. } = *program.root() else {
        panic!("fixture must produce one process machine root")
    };
    let body = program.function(entry).unwrap().body();
    assert!(body.addresses().any(|(_, address)| {
        address.steps().iter().any(|step| {
            matches!(
                step,
                crate::MachineAddressStep::Index {
                    index: crate::MachineIndex::Value(_),
                    stride: 4,
                    bound: crate::MachineIndexBound::Fixed(2),
                }
            )
        })
    }));
}

#[test]
fn outcome_switch_uses_layout_tag_offsets_and_frozen_tag_values() {
    let mir = lower_fixture(
        "func force(input: i32?): i32 { input! }\n\
         func main(): i32 { force(1) }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let switch = program.functions().find_map(|(_, function)| {
        function
            .body()
            .blocks()
            .find_map(|(_, block)| match block.terminator() {
                MachineTerminator::SwitchTag {
                    tag_offset, cases, ..
                } => Some((*tag_offset, cases)),
                _ => None,
            })
    });
    let (tag_offset, cases) = switch.expect("optional force must inspect one stored tag");
    assert_eq!(tag_offset, 0);
    assert!(
        cases
            .iter()
            .any(|case| case.value() == crate::MachineSwitchValue::Tag(0))
    );
}

#[test]
fn process_error_reporting_and_user_drop_are_closed_machine_operations() {
    let fallible = MachineProgram::lower(&lower_fixture("func main(): i32! { 1 }\n")).unwrap();
    assert!(fallible.functions().any(|(_, function)| {
        function.body().operations().any(|(_, operation)| {
            matches!(operation.kind(), MachineOperationKind::ReportError { .. })
        })
    }));

    let dropped_mir = lower_fixture(
        "struct Owned { value: i32 }\n\
         drop Owned(&+self) { return }\n\
         func main(): void {\n\
             let value = Owned { value: 1 }\n\
             return\n\
         }\n",
    );
    let dropped = MachineProgram::lower(&dropped_mir).unwrap();
    assert!(dropped.functions().any(|(_, function)| {
        function.body().operations().any(|(_, operation)| {
            matches!(operation.kind(), MachineOperationKind::InvokeDrop { .. })
        })
    }));
}

#[test]
fn region_lifetime_operations_reference_machine_values_and_stack_objects() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         func main(): void {\n\
             let allocator = Allocator {}\n\
             region temporary using allocator {}\n\
             return\n\
         }\n",
        &[&[]],
    );
    let mir = lower_selected_fixture(&fixture, false);
    let program = MachineProgram::lower(&mir).unwrap();
    let lifetime = program
        .functions()
        .flat_map(|(_, function)| function.body().operations())
        .filter_map(|(_, operation)| match operation.kind() {
            MachineOperationKind::CreateRegion { .. } => Some("create"),
            MachineOperationKind::ReleaseRegion { .. } => Some("release"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lifetime, ["create", "release"]);
}

#[test]
fn standard_primitives_keep_roles_and_use_the_shared_abi_planner() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.addr\n\
         use std/ptr.from_ref\n\
         func main(): usize {\n\
             let value: i32 = 7\n\
             addr(from_ref(&value))\n\
         }\n",
        &[&["ptr"], &["ptr"]],
    );
    let program = MachineProgram::lower(&lower_selected_fixture(&fixture, false)).unwrap();
    let calls = program
        .functions()
        .flat_map(|(_, function)| function.body().operations())
        .filter_map(|(_, operation)| {
            let MachineOperationKind::Call(call) = operation.kind() else {
                return None;
            };
            let crate::MachineCallTarget::Primitive(target) = call.target() else {
                return None;
            };
            Some((target.role(), target.abi().arguments().len()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        [
            (PrimitiveRole::PointerFromReference, 1),
            (PrimitiveRole::PointerAddress, 1),
        ]
    );
}

#[test]
fn literal_pack_is_a_dense_body_domain_and_uses_explicit_consumer_operations() {
    let mir = lower_fixture(
        "struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 let length = items.len()\n\
                 for item in items {}\n\
                 let _ = length\n\
                 return Self {}\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let values = Vec [1, 2]\n\
             drop values\n\
             0\n\
         }\n",
    );
    let program = MachineProgram::lower(&mir).unwrap();
    let (caller, pack_id) = program
        .functions()
        .find_map(|(_, function)| {
            function
                .body()
                .operations()
                .find_map(|(_, operation)| match operation.kind() {
                    MachineOperationKind::Call(call) => {
                        call.pack().map(|pack| (function.body(), pack))
                    }
                    _ => None,
                })
        })
        .expect("literal call must retain one pack identity");
    let pack = caller.pack(pack_id).unwrap();

    assert_eq!(caller.packs().len(), 1);
    assert_eq!(pack.segments().len(), 2);
    assert!(pack.segments().iter().all(|segment| {
        matches!(
            segment,
            crate::MachinePackSegment::Value {
                destruction: None,
                ..
            }
        )
    }));
    assert!(program.functions().any(|(_, function)| {
        let operations = function
            .body()
            .operations()
            .map(|(_, operation)| operation.kind())
            .collect::<Vec<_>>();
        operations
            .iter()
            .any(|kind| matches!(kind, MachineOperationKind::PackLength))
            && operations
                .iter()
                .any(|kind| matches!(kind, MachineOperationKind::PackNext))
            && operations
                .iter()
                .any(|kind| matches!(kind, MachineOperationKind::DestroyPack))
    }));
}

#[test]
fn spread_pack_freezes_iteration_and_residual_destruction_without_mir_members() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         struct Vec<T> {}\n\
         construct Vec<T> {\n\
             pub literal [](...items: T): Self {\n\
                 let _ = items.len()\n\
                 for item in items {}\n\
                 return Self {}\n\
             }\n\
         }\n\
         struct Leaf {}\n\
         drop Leaf(&+self) { return }\n\
         struct Item { leaf: Leaf }\n\
         struct Iter {}\n\
         conform Iterator for Iter {\n\
             type Item = Item\n\
             method &+self.next(): Item? { return none }\n\
         }\n\
         conform ExactSizeIterator for Iter {\n\
             method &self.remaining_len(): usize { return 0 }\n\
         }\n\
         drop Iter(&+self) { return }\n\
         func main(): i32 {\n\
             let iterator = Iter {}\n\
             let _ = Vec [Item { leaf: Leaf {} }, ...move iterator, Item { leaf: Leaf {} }]\n\
             0\n\
         }\n",
        &[&[], &[]],
    );
    let program = MachineProgram::lower(&lower_selected_fixture(&fixture, false)).unwrap();
    let pack = program
        .functions()
        .find_map(|(_, function)| function.body().packs().next().map(|(_, pack)| pack))
        .expect("spread literal must retain a machine pack");
    let [first, crate::MachinePackSegment::Spread(spread), last] = pack.segments() else {
        panic!("expected fixed, spread, fixed pack order")
    };

    for segment in [first, last] {
        let crate::MachinePackSegment::Value {
            destruction: Some(plan),
            ..
        } = segment
        else {
            panic!("owned fixed values must retain destruction")
        };
        assert!(matches!(
            plan.kind(),
            crate::MachineDestructionKind::Struct { fields, .. }
                if matches!(fields.as_ref(), [field]
                    if field.offset() == 0
                        && matches!(
                            field.plan().kind(),
                            crate::MachineDestructionKind::Struct { drop: Some(_), .. }
                        ))
        ));
    }
    assert_eq!(
        spread.contribution(),
        crate::MachinePackContribution::Direct
    );
    assert!(spread.destruction().is_some());
    assert!(matches!(
        spread.next().target(),
        crate::MachineCallTarget::Direct(_)
    ));
}

fn named_nominal(program: &nocter_mir::MirProgram, expected: &str) -> TypeId {
    let executable = program.executable();
    let graph = executable.target().checked().graph();
    executable
        .types()
        .iter()
        .find_map(|(ty, kind)| {
            let TypeKind::Nominal { definition, .. } = kind else {
                return None;
            };
            graph
                .declarations()
                .nominal_types()
                .get(*definition)
                .and_then(|nominal| graph.symbols().spelling(nominal.name()))
                .is_some_and(|name| name == expected)
                .then_some(ty)
        })
        .unwrap_or_else(|| panic!("missing nominal {expected}"))
}

fn borrow_type(types: &nocter_model::TypeStore, referent: BuiltinType) -> TypeId {
    let referent = types.builtin(referent);
    types
        .iter()
        .find_map(|(ty, kind)| {
            matches!(
                kind,
                TypeKind::Borrow {
                    capability: BorrowCapability::Readonly,
                    referent: actual,
                } if *actual == referent
            )
            .then_some(ty)
        })
        .unwrap()
}

fn lower_fixture(source: &str) -> nocter_mir::MirProgram {
    lower_selected_fixture(&CompilerFixture::with_app(source), false)
}

fn lower_test_fixture(source: &str) -> nocter_mir::MirProgram {
    lower_selected_fixture(&CompilerFixture::with_tests(source), true)
}

fn lower_selected_fixture(fixture: &CompilerFixture, tests: bool) -> nocter_mir::MirProgram {
    let (input, prelude) = fixture.input();
    let lowered = lower_compile_unit_declarations(&input, &prelude).unwrap();
    let (declarations, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, declarations, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared)
        .unwrap()
        .into_parts()
        .0;
    let standard_package = checked.graph().standard_package().unwrap();
    let registry = primitive_registry(&checked);
    let snapshot =
        ToolchainSnapshot::select(CompilationTarget::Arm64Darwin, standard_package, registry)
            .unwrap();
    let target = TargetProgram::build(checked, snapshot).unwrap();
    let selected = target
        .checked()
        .graph()
        .package_targets()
        .iter()
        .next()
        .unwrap()
        .0;
    let executable = if tests {
        ExecutableProgram::for_tests(target, selected).unwrap()
    } else {
        ExecutableProgram::for_executable(target, selected).unwrap()
    };
    lower_executable(executable).unwrap()
}

fn primitive_registry(checked: &nocter_checking::CheckedProgram) -> PrimitiveRegistry {
    let graph = checked.graph();
    PrimitiveRegistry::new(PrimitiveRole::ALL.iter().copied().map(|role| {
        let callable = graph
            .declarations()
            .callables()
            .iter()
            .find_map(|(callable, declaration)| {
                let CallableOwner::Module(module) = declaration.owner() else {
                    return None;
                };
                let actual_path = graph
                    .modules()
                    .get(module)?
                    .path()
                    .segments()
                    .iter()
                    .map(|segment| graph.symbols().spelling(*segment))
                    .collect::<Option<Vec<_>>>()?;
                (declaration.kind() == CallableKind::Primitive
                    && actual_path == role.module_path()
                    && declaration
                        .name()
                        .and_then(|name| graph.symbols().spelling(name))
                        == Some(role.declaration_name()))
                .then_some(callable)
            })
            .unwrap_or_else(|| panic!("missing fixture primitive {role:?}"));
        PrimitiveBinding::new(role, callable)
    }))
    .unwrap()
}
