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
    MachineEndianness, MachineLayoutKind, MachineLayoutStore, MachineOutcomeKind, MachineScalar,
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
    let fixture = CompilerFixture::with_app(source);
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
    let executable = ExecutableProgram::for_executable(target, selected).unwrap();
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
