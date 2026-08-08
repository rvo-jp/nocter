use super::*;

#[test]
fn lowers_pointer_from_addr_aggregate_field_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"use std/text.make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text/index.nct",
                r#"use std/ptr.from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    return Text { ptr: from_addr(1), len: 2, capacity: 3 }
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 8,
                value: usize_const(2),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Return,
                offset: 16,
                value: usize_const(3),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointer_from_addr_aggregate_field_binding_return() {
    let function = lower_imported_named_function_with_nocter_home_files(
        r#"use std/text.make

func main(): i32 {
    return 0
}
"#,
        "make",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/text/index.nct",
                r#"use std/ptr.from_addr

pub struct Text {
    ptr: *u8
    len: usize
    capacity: usize
}

pub func make(): Text {
    let value = Text { ptr: from_addr(1), len: 2, capacity: 3 }
    return move value
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "make");
    assert!(matches!(
        function.target,
        CallTarget::Imported { ref name, .. } if name == "make"
    ));
    assert_eq!(
        function.return_type,
        Type::Aggregate {
            layout: ValueLayout::new(24, 8),
        }
    );
    assert_eq!(
        function.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: ValueLayout::new(24, 8),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 0,
                value: usize_const(1),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 8,
                value: usize_const(2),
            },
            Instruction::StoreAggregateUsize {
                destination: AggregateLocation::Slot(0),
                offset: 16,
                value: usize_const(3),
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Return,
                source: AggregateLocation::Slot(0),
                layout: ValueLayout::new(24, 8),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointer_from_ref_scalar_borrow_parameter_binding_return() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func address_of(value: &u8): usize {
    let pointer = from_ref(value)
    return addr(pointer)
}

func main(): i32 {
    return 0
}
"#,
        "address_of",
        &[(
            "std/ptr/index.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );
    assert_eq!(
        function,
        Function {
            name: "address_of".to_string(),
            target: CallTarget::same_file("address_of"),
            return_type: Type::Usize,
            instructions: vec![
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(0),
                    source: BorrowSource::BorrowParameter(0),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Return,
                    value: UsizeValue::Location(UsizeLocation::Local(0)),
                },
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_pointer_returning_normal_call_as_an_address_word() {
    let function = lower_imported_named_function_with_nocter_home_files(
        "use std/pointer_calls.address\nfunc main(): i32 { return 0 }\n",
        "address",
        &[
            (
                "std/ptr/index.nct",
                r#"pub primitive addr<T>(pointer: *T): usize
pub(nocter) primitive from_addr<T>(address: usize): *T
"#,
            ),
            (
                "std/pointer_calls/index.nct",
                r#"use std/ptr.{addr, from_addr}

func pointer(): *u8 {
    return from_addr(7)
}

pub func address(): usize {
    return addr(pointer())
}
"#,
            ),
        ],
    );

    assert_eq!(function.name, "address");
    assert_eq!(function.return_type, Type::Usize);
    assert!(matches!(
        function.instructions.as_slice(),
        [
            Instruction::CallUsize {
                destination: UsizeLocation::Local(0),
                target,
                arguments,
            },
            Instruction::SetUsize {
                destination: UsizeLocation::Return,
                value: UsizeValue::Location(UsizeLocation::Local(0)),
            },
            Instruction::Return,
        ] if call_target_name_is(target, "pointer") && arguments.is_empty()
    ));
}

#[test]
fn lowers_pointer_from_ref_local_borrow_binding() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func main(): i32 {
    let value: u8 = 1
    let pointer = from_ref(&value)
    let address: usize = addr(pointer)
    return 0
}
"#,
        "main",
        &[(
            "std/ptr/index.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: u8_const(1),
                },
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(1),
                    source: BorrowSource::U8(U8Location::Local(0)),
                },
                Instruction::SetUsize {
                    destination: UsizeLocation::Local(2),
                    value: UsizeValue::Location(UsizeLocation::Local(1)),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_pointer_from_ref_direct_addr_local_borrow() {
    let function = lower_named_function_with_nocter_home_files(
        r#"use std/ptr.{addr, from_ref}

func main(): i32 {
    let value: u8 = 1
    let address: usize = addr(from_ref(&value))
    return 0
}
"#,
        "main",
        &[(
            "std/ptr/index.nct",
            r#"pub primitive addr<T>(pointer: *T): usize
pub primitive from_ref<T>(value: &T): *T
"#,
        )],
    );

    assert_eq!(
        function,
        Function {
            name: "main".to_string(),
            target: CallTarget::same_file("main"),
            return_type: Type::I32,
            instructions: vec![
                Instruction::SetU8 {
                    destination: U8Location::Local(0),
                    value: u8_const(1),
                },
                Instruction::SetUsizeFromBorrow {
                    destination: UsizeLocation::Local(1),
                    source: BorrowSource::U8(U8Location::Local(0)),
                },
                set_return_i32(0),
                Instruction::Return,
            ],
        }
    );
}

#[test]
fn lowers_aggregate_pointer_never_call_as_normal_call_then_trap() {
    let aggregate_type = Type::Aggregate {
        layout: ValueLayout::new(24, 8),
    };
    let function = lower_named_function_with_signatures(
        r#"copy struct Big {
    first: usize
    second: usize
    code: usize
}

func main(): i32 {
    let value = Big { first: 1, second: 2, code: 42 }
    return abort(value)
}

func abort(value: Big): never {
    abort(value)
}
"#,
        "main",
        function_signatures(vec![("abort", Type::Never, vec![aggregate_type.clone()])]),
    )
    .unwrap();

    assert!(
        function.instructions.contains(&Instruction::CallVoid {
            target: CallTarget::same_file("abort"),
            arguments: vec![ScalarArgument::AggregateIndirect(AggregateArgument {
                source: AggregateArgumentSource::Slot(0),
            })],
        }),
        "{function:?}"
    );
    assert_eq!(function.instructions.last(), Some(&Instruction::Trap));
}

#[test]
fn lowers_close_fd_raw_call() {
    let close = lower_imported_named_function_with_nocter_home_files(
        r#"use std/io_close.close_raw

func main(): void {
    return
}
"#,
        "close_raw",
        &[
            std_io_file(),
            (
                "std/io_close/index.nct",
                r#"use std/io.close_fd_raw

pub func close_raw(fd: i32): void {
    close_fd_raw(fd)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        close.instructions,
        vec![
            Instruction::CloseFd {
                fd: I32Value::Location(I32Location::Parameter(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_u8_to_ptr_call() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_nul

func main(): void {
    return
}
"#,
        "store_nul",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_u8_to_ptr(destination: *u8, offset: usize, value: u8): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_u8_to_ptr

pub func store_nul(address: usize, offset: usize): void {
    store_u8_to_ptr(from_addr(address), offset, 0)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreU8ToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: U8Value::Const(0),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_copy_ptr_to_ptr_call() {
    let copy = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_copy.copy

func main(): void {
    return
}
"#,
        "copy",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive copy_ptr_to_ptr(destination: *u8, source: *u8, byte_count: usize): void
"#,
            ),
            (
                "std/ptr_copy/index.nct",
                r#"use std/ptr.copy_ptr_to_ptr
use std/ptr.from_addr

pub func copy(destination: usize, source: usize, byte_count: usize): void {
    copy_ptr_to_ptr(from_addr(destination), from_addr(source), byte_count)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        copy.instructions,
        vec![
            Instruction::CopyPointerBytes {
                destination: UsizeValue::Location(UsizeLocation::Parameter(0)),
                source: UsizeValue::Location(UsizeLocation::Parameter(1)),
                byte_count: UsizeValue::Location(UsizeLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_usize() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_word

func main(): void {
    return
}
"#,
        "store_word",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_word(address: usize, offset: usize, value: usize): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreUsizeToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: UsizeValue::Location(UsizeLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_borrow() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_borrow

func main(): void {
    return
}
"#,
        "store_borrow",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

struct Item { value: i32 }

pub func store_borrow(address: usize, offset: usize, value: &Item): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert!(
        store
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::SetUsizeFromBorrow { .. }))
    );
    assert!(
        store
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, Instruction::StoreUsizeToPointer { .. }))
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_i32() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_number

func main(): void {
    return
}
"#,
        "store_number",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_number(address: usize, offset: usize, value: i32): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreI32ToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: I32Value::Location(I32Location::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_bool() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_flag

func main(): void {
    return
}
"#,
        "store_flag",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_flag(address: usize, offset: usize, value: bool): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreBoolToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: BoolValue::Location(BoolLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_str() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_text

func main(): void {
    return
}
"#,
        "store_text",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_text(address: usize, offset: usize, value: &str): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    assert_eq!(
        store.instructions,
        vec![
            Instruction::StoreStrToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                value: StrValue::Location(StrLocation::Parameter(2)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_copy_aggregate() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_pair

func main(): void {
    return
}
"#,
        "store_pair",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

copy struct Pair {
    value: i32
}

pub func store_pair(address: usize, offset: usize, value: Pair): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    let pair_layout = ValueLayout { size: 4, align: 4 };
    assert_eq!(
        store.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: pair_layout,
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 2 },
                layout: pair_layout,
            },
            Instruction::CopyAggregateToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                source: AggregateLocation::Slot(0),
                layout: pair_layout,
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_store_value_to_ptr_call_for_fixed_array() {
    let store = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_store.store_pair

func main(): void {
    return
}
"#,
        "store_pair",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive store_value_to_ptr<T>(destination: *T, offset: usize, value: T): void
"#,
            ),
            (
                "std/ptr_store/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.store_value_to_ptr

pub func store_pair(address: usize, offset: usize, value: [i32; 2]): void {
    store_value_to_ptr(from_addr(address), offset, value)
    return
}
"#,
            ),
        ],
    );

    let pair_layout = ValueLayout { size: 8, align: 4 };
    assert_eq!(
        store.instructions,
        vec![
            Instruction::ReserveAggregateSlot {
                slot_index: 0,
                layout: pair_layout,
            },
            Instruction::CopyAggregate {
                destination: AggregateLocation::Slot(0),
                source: AggregateLocation::DirectParameter { start_index: 2 },
                layout: pair_layout,
            },
            Instruction::CopyAggregateToPointer {
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                source: AggregateLocation::Slot(0),
                layout: pair_layout,
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_drop_value_at_ptr_call_for_owned_aggregate() {
    let drop_at = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_drop.drop_at

func main(): void {
    return
}
"#,
        "drop_at",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive drop_value_at_ptr<T>(pointer: *T, offset: usize): void
"#,
            ),
            (
                "std/ptr_drop/index.nct",
                r#"use std/ptr.drop_value_at_ptr

struct Item {
    value: i32
}

impl Item {
    drop &+self {
        return
    }
}

pub func drop_at(pointer: *Item, offset: usize): void {
    drop_value_at_ptr(pointer, offset)
    return
}
"#,
            ),
        ],
    );
    let drop_source = match drop_at.target {
        CallTarget::Imported { source, .. } => source,
        CallTarget::SameFile(_) => panic!("expected imported pointer-drop helper"),
    };

    assert_eq!(
        drop_at.instructions,
        vec![
            Instruction::CallVoid {
                target: CallTarget::imported(drop_source, "Item.drop"),
                arguments: vec![ScalarArgument::Borrow(BorrowArgument {
                    source: BorrowSource::PointerOffset {
                        pointer: UsizeLocation::Parameter(0),
                        offset: UsizeLocation::Parameter(1),
                        field_offset: 0,
                    },
                })],
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_take_value_at_ptr_call_for_owned_aggregate_return() {
    let take_at = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_take.take_at

func main(): void {
    return
}
"#,
        "take_at",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive take_value_at_ptr<T>(pointer: *T, offset: usize): T
"#,
            ),
            (
                "std/ptr_take/index.nct",
                r#"use std/ptr.take_value_at_ptr

struct Item {
    value: i32
}

impl Item {
    drop &+self {
        return
    }
}

pub func take_at(pointer: *Item, offset: usize): Item {
    return take_value_at_ptr(pointer, offset)
}
"#,
            ),
        ],
    );

    assert_eq!(
        take_at.instructions,
        vec![
            Instruction::CopyPointerToAggregate {
                destination: AggregateLocation::DirectReturn,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                offset: UsizeValue::Location(UsizeLocation::Parameter(1)),
                layout: ValueLayout::new(4, 4),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_pointee_size_call_for_usize_pointer_field() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_size.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive pointee_size<T>(pointer: *T): usize
"#,
            ),
            (
                "std/ptr_size/index.nct",
                r#"use std/ptr.pointee_size

pub copy struct Holder {
    pub ptr: *usize
}

pub func size(holder: Holder): usize {
    return pointee_size(holder.ptr)
}
"#,
            ),
        ],
    );

    assert!(
        size.instructions.contains(&Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Const(8),
        }),
        "{:?}",
        size.instructions
    );
    assert!(
        !size.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallUsize { target, .. } if call_target_name_is(target, "pointee_size")
        )),
        "{:?}",
        size.instructions
    );
}

#[test]
fn lowers_pointee_size_call_for_u8_pointer_field() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_size.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive pointee_size<T>(pointer: *T): usize
"#,
            ),
            (
                "std/ptr_size/index.nct",
                r#"use std/ptr.pointee_size

pub copy struct Holder {
    pub ptr: *u8
}

pub func size(holder: Holder): usize {
    return pointee_size(holder.ptr)
}
"#,
            ),
        ],
    );

    assert!(
        size.instructions.contains(&Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Const(1),
        }),
        "{:?}",
        size.instructions
    );
}

#[test]
fn lowers_pointee_align_call_from_concrete_aggregate_layout() {
    let align = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_align.align

func main(): void {
    return
}
"#,
        "align",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive pointee_align<T>(pointer: *T): usize
"#,
            ),
            (
                "std/ptr_align/index.nct",
                r#"use std/ptr.pointee_align

pub copy struct Pair {
    pub byte: u8
    pub word: usize
}

pub copy struct Holder {
    pub ptr: *Pair
}

pub func align(holder: Holder): usize {
    return pointee_align(holder.ptr)
}
"#,
            ),
        ],
    );

    assert!(
        align.instructions.contains(&Instruction::SetUsize {
            destination: UsizeLocation::Return,
            value: UsizeValue::Const(8),
        }),
        "{:?}",
        align.instructions
    );
    assert!(
        !align.instructions.iter().any(|instruction| matches!(
            instruction,
            Instruction::CallUsize { target, .. } if call_target_name_is(target, "pointee_align")
        )),
        "{:?}",
        align.instructions
    );
}

#[test]
fn lowers_slice_from_raw_parts_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    return slice_from_raw_parts_mut(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_str_from_raw_parts_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_str.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive str_from_raw_parts(pointer: *u8, len: usize): &str from static
"#,
            ),
            (
                "std/ptr_str/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.str_from_raw_parts

pub func view(address: usize, len: usize): &str from static {
    let text = str_from_raw_parts(from_addr(address), len)
    return text
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetStrRawParts {
                destination: StrLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetStr {
                destination: StrLocation::Return,
                value: StrValue::Location(StrLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_slice_from_raw_parts_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    let view = slice_from_raw_parts_mut(from_addr(address), len)
    return view
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: SliceValue::Location(SliceLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_value_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value

pub func view(address: usize, len: usize): &[u8] {
    return slice_from_raw_parts_value(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_inferred_slice_from_raw_parts_value_local_binding() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view

func main(): void {
    return
}
"#,
        "view",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value<T>(pointer: *T, len: usize): &[T]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value

pub func view(address: usize, len: usize): &[u8] {
    let slice = slice_from_raw_parts_value(from_addr(address), len)
    return slice
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetSlice {
                destination: SliceLocation::Return,
                value: SliceValue::Location(SliceLocation::Local(0)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_value_mut_call() {
    let view = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.view_mut

func main(): void {
    return
}
"#,
        "view_mut",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_value_mut<T>(pointer: *T, len: usize): &+[T]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_value_mut

pub func view_mut(address: usize, len: usize): &+[u8] {
    return slice_from_raw_parts_value_mut(from_addr(address), len)
}
"#,
            ),
        ],
    );

    assert_eq!(
        view.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Return,
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::Return,
        ]
    );
}

#[test]
fn lowers_str_from_raw_parts_call_len_return() {
    let size = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_str.size

func main(): void {
    return
}
"#,
        "size",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive str_from_raw_parts(pointer: *u8, len: usize): &str
"#,
            ),
            (
                "std/ptr_str/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.str_from_raw_parts

pub func size(address: usize, len: usize): usize {
    return str_from_raw_parts(from_addr(address), len).len()
}
"#,
            ),
        ],
    );

    assert_eq!(
        size.instructions,
        vec![
            Instruction::SetStrRawParts {
                destination: StrLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::TailCall {
                target: builtin_str_method_target("len"),
                arguments: vec![ScalarArgument::Str(StrValue::Location(StrLocation::Local(
                    0
                ),))],
            },
        ]
    );
}

#[test]
fn lowers_slice_from_raw_parts_call_index_return() {
    let first = lower_imported_named_function_with_nocter_home_files(
        r#"use std/ptr_slice.first

func main(): void {
    return
}
"#,
        "first",
        &[
            (
                "std/ptr/index.nct",
                r#"pub(nocter) primitive from_addr<T>(address: usize): *T
pub(nocter) primitive slice_from_raw_parts_mut(pointer: *u8, len: usize): &+[u8]
"#,
            ),
            (
                "std/ptr_slice/index.nct",
                r#"use std/ptr.from_addr
use std/ptr.slice_from_raw_parts_mut

pub func first(address: usize, len: usize): u8 {
    return slice_from_raw_parts_mut(from_addr(address), len)[0]
}
"#,
            ),
        ],
    );

    assert_eq!(
        first.instructions,
        vec![
            Instruction::SetSliceRawParts {
                destination: SliceLocation::Local(0),
                pointer: UsizeValue::Location(UsizeLocation::Parameter(0)),
                len: UsizeValue::Location(UsizeLocation::Parameter(1)),
            },
            Instruction::SetU8 {
                destination: U8Location::Return,
                value: U8Value::SliceIndex {
                    source: SliceLocation::Local(0),
                    index: usize_const(0),
                },
            },
            Instruction::Return,
        ]
    );
}
