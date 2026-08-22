use nocter_checking::{check_prepared_program, prepare_program_checking};
use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_declarations::{CallableKind, CallableOwner};
use nocter_machine::MachineProgram;
use nocter_mir::lower_executable;
use nocter_model::CompilationTarget;
use nocter_runtime_contract::{PrimitiveBinding, PrimitiveRegistry, PrimitiveRole};
use nocter_target_program::{ExecutableProgram, TargetProgram, ToolchainSnapshot};
use nocter_test_support::CompilerFixture;

#[test]
fn constant_process_crosses_the_complete_native_pipeline() {
    let machine = lower_machine("func main(): i32 { 42 }\n");
    let first = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let second = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let first_image = nocter_macho::MachOImage::build(&first).unwrap();
    let second_image = nocter_macho::MachOImage::build(&second).unwrap();

    assert_eq!(first_image, second_image);
    assert!(!first_image.bytes().is_empty());
    execute_and_assert_status(&first_image, 42);
}

#[test]
fn independent_test_roots_cross_the_complete_native_pipeline() {
    let machine = lower_test_machine(
        "test passes { return }\n\
         test fails { return error.new(\"app.failure\", \"failed\") }\n",
    );
    let suite = nocter_arm64::Arm64TestSuite::lower_machine(&machine).unwrap();

    assert_eq!(
        suite
            .tests()
            .iter()
            .map(nocter_arm64::Arm64TestExecutable::name)
            .collect::<Vec<_>>(),
        ["passes", "fails"]
    );
    let passing = nocter_macho::MachOImage::build(suite.tests()[0].program()).unwrap();
    let failing = nocter_macho::MachOImage::build(suite.tests()[1].program()).unwrap();
    execute_and_assert_output(&passing, 0, b"");
    execute_and_assert_output(&failing, 1, b"app.failure: failed\n");
}

#[test]
fn scalar_call_and_arithmetic_cross_the_complete_native_pipeline() {
    let machine = lower_machine(
        "func double(value: i32): i32 { value * 2 }\n\
         func main(): i32 { double(20) + 2 }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn fixed_literal_pack_callbacks_cross_the_complete_native_pipeline() {
    let machine = lower_machine(
        "struct Sum { value: i32 }\n\
         construct Sum {\n\
             pub literal [](...items: i32): Self {\n\
                 var total = 0\n\
                 for item in items { total += item }\n\
                 return Self { value: total }\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let result = Sum [10, 20, 12]\n\
             return result.value\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn fixed_literal_pack_residual_cleanup_uses_generated_destruction() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         struct Sink {}\n\
         construct Sink {\n\
             pub literal [](...items: ExitOnDrop): Self { return Self {} }\n\
         }\n\
         func main(): i32 {\n\
             let value = Sink [ExitOnDrop { status: 42 }]\n\
             drop value\n\
             return 1\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn spread_literal_pack_callbacks_cross_the_complete_native_pipeline() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         use std/mem.allocation_context_state_for_test\n\
         struct Sum { value: i32 }\n\
         construct Sum {\n\
             pub literal [](...items: i32): Self {\n\
                 var total = 0\n\
                 for item in items { total += item }\n\
                 return Self { value: total }\n\
             }\n\
         }\n\
         struct Iter { next_value: i32\n\
             remaining: usize }\n\
         conform Iterator for Iter {\n\
             type Item = i32\n\
             method &+self.next(): i32? {\n\
                 let _ = allocation_context_state_for_test()\n\
                 if self.remaining == 0 {\n\
                     return none\n\
                 }\n\
                 let value = self.next_value\n\
                 self.next_value += 1\n\
                 self.remaining -= 1\n\
                 return value\n\
             }\n\
         }\n\
         conform ExactSizeIterator for Iter {\n\
             method &self.remaining_len(): usize { return self.remaining }\n\
         }\n\
         func main(): i32 {\n\
             let iterator = Iter { next_value: 4, remaining: 3 }\n\
             let empty = Iter { next_value: 100, remaining: 0 }\n\
             let result = Sum [10, ...move iterator, ...move empty, 20]\n\
             return result.value\n\
         }\n",
        &[&[], &[], &["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 45);
}

#[test]
fn spread_literal_pack_residual_cleanup_destroys_the_iterator() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         use std/process.exit_for_test\n\
         struct Sink {}\n\
         construct Sink {\n\
             pub literal [](...items: i32): Self { return Self {} }\n\
         }\n\
         struct Iter {}\n\
         conform Iterator for Iter {\n\
             type Item = i32\n\
             method &+self.next(): i32? { return none }\n\
         }\n\
         conform ExactSizeIterator for Iter {\n\
             method &self.remaining_len(): usize { return 0 }\n\
         }\n\
         drop Iter(&+self) { exit_for_test(42) }\n\
         func main(): i32 {\n\
             let iterator = Iter {}\n\
             let value = Sink [...move iterator]\n\
             drop value\n\
             return 1\n\
         }\n",
        &[&[], &[], &["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn spread_literal_pack_transports_indirect_optional_values() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         copy struct Large { first: i64\n\
             second: i64\n\
             third: i32 }\n\
         struct Sum { value: i32 }\n\
         construct Sum {\n\
             pub literal [](...items: Large): Self {\n\
                 var total = 0\n\
                 for item in items { total += item.third }\n\
                 return Self { value: total }\n\
             }\n\
         }\n\
         struct Iter { value: Large\n\
             remaining: usize }\n\
         conform Iterator for Iter {\n\
             type Item = Large\n\
             method &+self.next(): Large? {\n\
                 if self.remaining == 0 {\n\
                     return none\n\
                 }\n\
                 self.remaining -= 1\n\
                 return self.value\n\
             }\n\
         }\n\
         conform ExactSizeIterator for Iter {\n\
             method &self.remaining_len(): usize { return self.remaining }\n\
         }\n\
         func main(): i32 {\n\
             let iterator = Iter {\n\
                 value: Large { first: 10, second: 20, third: 42 },\n\
                 remaining: 1,\n\
             }\n\
             let result = Sum [...move iterator]\n\
             return result.value\n\
         }\n",
        &[&[], &[]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn spread_literal_pack_copies_borrowed_iterator_items() {
    let fixture = CompilerFixture::with_app_iteration_standard_uses(
        "use std.Iterator\n\
         use std.ExactSizeIterator\n\
         struct Sum { value: i32 }\n\
         construct Sum {\n\
             pub literal [](...items: i32): Self {\n\
                 var total = 0\n\
                 for item in items { total += item }\n\
                 return Self { value: total }\n\
             }\n\
         }\n\
         struct Source { first: i32\n\
             second: i32 }\n\
         struct RefIter { source: &Source\n\
             index: usize }\n\
         instance Source {\n\
             pub operator (...&self): RefIter from self {\n\
                 return RefIter { source: self, index: 0 }\n\
             }\n\
         }\n\
         conform Iterator for RefIter {\n\
             type Item = &i32\n\
             method &+self.next(): &i32? from self {\n\
                 if self.index == 0 {\n\
                     self.index = 1\n\
                     return &self.source.first\n\
                 }\n\
                 if self.index == 1 {\n\
                     self.index = 2\n\
                     return &self.source.second\n\
                 }\n\
                 return none\n\
             }\n\
         }\n\
         conform ExactSizeIterator for RefIter {\n\
             method &self.remaining_len(): usize { return 2 - self.index }\n\
         }\n\
         func main(): i32 {\n\
             let source = Source { first: 20, second: 21 }\n\
             let result = Sum [1, ...source]\n\
             return result.value\n\
         }\n",
        &[&[], &[]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn scalar_comparison_and_branch_cross_the_complete_native_pipeline() {
    let machine = lower_machine(
        "func main(): i32 {\n\
             if !(3 < 4) { return 1 }\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn narrow_signed_values_cross_calls_and_borrowed_comparisons() {
    let machine = lower_machine(
        "func ordered(left: i8, right: i8): bool { left < right }\n\
         func main(): i32 {\n\
             if !ordered(-2, 1) { return 1 }\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn guarded_signed_arithmetic_crosses_the_complete_native_pipeline() {
    let machine = lower_machine("func main(): i32 { ((-21 / -3) << 3) - (7 % 4) - 11 }\n");
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn block_parameters_cross_control_flow_edges_natively() {
    let machine = lower_machine(
        "func choose(condition: bool): i32 {\n\
             if condition { 1 } else { 2 }\n\
         }\n\
         func main(): i32 { choose(true) * 40 + choose(false) }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn memory_values_cross_control_flow_edges_natively() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func choose(condition: bool): Large {\n\
             if condition {\n\
                 Large { first: 20, second: 20, third: 42 }\n\
             } else {\n\
                 Large { first: 1, second: 1, third: 1 }\n\
             }\n\
         }\n\
         func main(): i32 {\n\
             let fallback = choose(false)\n\
             if fallback.third == 1 {\n\
                 let selected = choose(true)\n\
                 return selected.third\n\
             }\n\
             return 2\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn optional_tag_switches_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func force(value: i32?): i32 { value! }\n\
         func main(): i32 { force(42) }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn static_text_and_two_word_view_transport_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func relay(text: &str): &str { text }\n\
         func main(): i32 {\n\
             let text = relay(\"hello\")\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn two_word_views_cross_the_register_window_and_outgoing_stack() {
    let machine = lower_machine(
        "func last(\n\
             a: &str, b: &str, c: &str, d: &str, e: &str,\n\
             f: &str, g: &str, h: &str, i: &str,\n\
         ): &str { i }\n\
         func main(): i32 {\n\
             let text = last(\"a\", \"b\", \"c\", \"d\", \"e\", \"f\", \"g\", \"h\", \"i\")\n\
             return 42\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn layout_owned_aggregate_bytes_cross_the_native_pipeline() {
    let machine = lower_machine(
        "copy struct Triple { first: u8\n    second: u8\n    third: u8 }\n\
         copy struct Direct { first: i32\n    second: i32\n    third: i32 }\n\
         copy struct Large { first: i64\n    second: i64\n    third: i64 }\n\
         func main(): i32 {\n\
             let triple = Triple { first: 1, second: 2, third: 3 }\n\
             let direct = Direct { first: 20, second: 20, third: 2 }\n\
             let large = Large { first: 1, second: 2, third: 42 }\n\
             if triple.third == 3 {\n\
                 if large.third == 42 {\n\
                     return direct.first + direct.second + direct.third\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn indirect_aggregate_arguments_and_results_cross_the_native_pipeline() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func identity(value: Large): Large { value }\n\
         func relay(value: Large): Large { identity(value) }\n\
         func main(): i32 {\n\
             let value = relay(Large { first: 1, second: 2, third: 42 })\n\
             return value.third\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn indirect_aggregate_arguments_cross_the_outgoing_stack_boundary() {
    let machine = lower_machine(
        "copy struct Large { first: i64\n    second: i64\n    third: i32 }\n\
         func ninth(\n\
             a: Large, b: Large, c: Large, d: Large, e: Large,\n\
             f: Large, g: Large, h: Large, i: Large,\n\
         ): Large { i }\n\
         func main(): i32 {\n\
             let value = ninth(\n\
                 Large { first: 0, second: 0, third: 1 },\n\
                 Large { first: 0, second: 0, third: 2 },\n\
                 Large { first: 0, second: 0, third: 3 },\n\
                 Large { first: 0, second: 0, third: 4 },\n\
                 Large { first: 0, second: 0, third: 5 },\n\
                 Large { first: 0, second: 0, third: 6 },\n\
                 Large { first: 0, second: 0, third: 7 },\n\
                 Large { first: 0, second: 0, third: 8 },\n\
                 Large { first: 0, second: 0, third: 42 },\n\
             )\n\
             return value.third\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn allocation_context_crosses_root_and_nested_call_boundaries() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/mem.allocation_context_state_for_test\n\
         use std/mem.allocation_context_kind_for_test\n\
         func leaf(): usize {\n\
             allocation_context_state_for_test() + allocation_context_kind_for_test()\n\
         }\n\
         func middle(): usize { leaf() }\n\
         func main(): i32 {\n\
             if middle() == 0 { return 42 }\n\
             return 1\n\
         }\n",
        &[&["mem"], &["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn lexical_regions_select_and_restore_non_movable_contexts() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         use std/mem.allocation_context_state_for_test\n\
         use std/mem.allocation_context_kind_for_test\n\
         func current_state(): usize { allocation_context_state_for_test() }\n\
         func main(): i32 {\n\
             let allocator = Allocator { state: 0, kind: 0 }\n\
             region outer using allocator {\n\
                 let outer_state = current_state()\n\
                 if outer_state == 0 { return 1 }\n\
                 if allocation_context_kind_for_test() != 1 { return 2 }\n\
                 region inner using outer {\n\
                     let inner_state = current_state()\n\
                     if inner_state == 0 { return 3 }\n\
                     if inner_state == outer_state { return 4 }\n\
                     if allocation_context_kind_for_test() != 1 { return 5 }\n\
                 }\n\
                 if current_state() != outer_state { return 6 }\n\
                 return 42\n\
             }\n\
         }\n",
        &[&[], &["mem"], &["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn lexical_region_context_reaches_authored_destruction() {
    let fixture = CompilerFixture::with_app_allocation_standard_uses(
        "use std.Allocator\n\
         use std/mem.allocation_context_kind_for_test\n\
         use std/process.exit_for_test\n\
         struct Guard {}\n\
         drop Guard(&+self) {\n\
             if allocation_context_kind_for_test() != 1 {\n\
                 exit_for_test(7)\n\
             }\n\
             return\n\
         }\n\
         func main(): i32 {\n\
             let allocator = Allocator { state: 0, kind: 0 }\n\
             region temporary using allocator {\n\
                 let guard = Guard {}\n\
             }\n\
             return 42\n\
         }\n",
        &[&[], &["mem"], &["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn pointer_and_view_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.{\n\
             addr, from_ref, pointee_size_for_test, pointee_align_for_test,\n\
         }\n\
         use std/str.{str_len_for_test, str_ptr_addr_for_test}\n\
         use std/slice.{slice_len_for_test, slice_ptr_addr_for_test}\n\
         use std/string.{bytes_from_str_for_test, str_subview_unchecked_for_test}\n\
         func main(): i32 {\n\
             let byte: u8 = 65\n\
             let pointer = from_ref(&byte)\n\
             let address = addr(pointer)\n\
             let middle = str_subview_unchecked_for_test(\"hello\", 1, 3)\n\
             let static_bytes = bytes_from_str_for_test(\"hello\")\n\
             if address == addr(pointer) {\n\
                 if pointee_size_for_test(pointer) == 1 {\n\
                     if pointee_align_for_test(pointer) == 1 {\n\
                         if str_len_for_test(middle) == 3 {\n\
                             if slice_len_for_test(static_bytes) == 5 {\n\
                                 if slice_ptr_addr_for_test(static_bytes)\n\
                                     == str_ptr_addr_for_test(\"hello\") {\n\
                                     return 42\n\
                                 }\n\
                             }\n\
                         }\n\
                     }\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["ptr"], &["str"], &["slice"], &["string"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn memory_transfer_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.{\n\
             copy_ptr_to_ptr_for_test, copy_str_to_ptr_for_test, from_ref, from_ref_mut,\n\
             store_u8_to_ptr_for_test, store_value_to_ptr_for_test,\n\
             take_three_u64_at_ptr_for_test, take_u64_at_ptr_for_test,\n\
         }\n\
         struct Bytes {\n\
             a: u8\n\
             b: u8\n\
             c: u8\n\
             d: u8\n\
             e: u8\n\
         }\n\
         struct Large {\n\
             first: u64\n\
             second: u64\n\
             third: u64\n\
         }\n\
         struct LargePair {\n\
             first: Large\n\
             second: Large\n\
         }\n\
         func main(): i32 {\n\
             var bytes = Bytes { a: 0, b: 0, c: 0, d: 0, e: 0 }\n\
             copy_str_to_ptr_for_test(from_ref_mut(&+bytes.a), 0, \"hello\")\n\
             copy_str_to_ptr_for_test(from_ref_mut(&+bytes.a), 2, \"xy\")\n\
             var copied = Bytes { a: 0, b: 0, c: 0, d: 0, e: 0 }\n\
             copy_ptr_to_ptr_for_test(\n\
                 from_ref_mut(&+copied.a),\n\
                 from_ref(&bytes.a),\n\
                 5,\n\
             )\n\
             store_u8_to_ptr_for_test(from_ref_mut(&+copied.a), 1, 97)\n\
             var pair = LargePair {\n\
                 first: Large { first: 10, second: 20, third: 30 },\n\
                 second: Large { first: 0, second: 0, third: 0 },\n\
             }\n\
             let replacement = Large { first: 40, second: 41, third: 42 }\n\
             let large_pointer = from_ref_mut(&+pair.first)\n\
             store_value_to_ptr_for_test(large_pointer, 24, move replacement)\n\
             let recovered = take_u64_at_ptr_for_test(from_ref(&pair.second.third), 0)\n\
             var arrays: [[u64; 3]; 2] = [[1, 2, 3], [40, 41, 42]]\n\
             let recovered_array = take_three_u64_at_ptr_for_test(\n\
                 from_ref_mut(&+arrays[0]),\n\
                 24,\n\
             )\n\
             if bytes.a == 104 {\n\
                 if bytes.e == 111 {\n\
                     if copied.a == 104 {\n\
                         if copied.b == 97 {\n\
                             if copied.c == 120 {\n\
                                 if copied.d == 121 {\n\
                                     if copied.e == 111 {\n\
                                         if recovered == 42 {\n\
                                             if recovered_array[2] == 42 {\n\
                                                 return 42\n\
                                             }\n\
                                         }\n\
                                     }\n\
                                 }\n\
                             }\n\
                         }\n\
                     }\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn darwin_syscall_primitives_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.{syscall0_succeeds_for_test, syscall1_fails_for_test}\n\
         func main(): i32 {\n\
             if syscall0_succeeds_for_test(0x02000014) {\n\
                 if syscall1_fails_for_test(0x02000006, 18446744073709551615) {\n\
                     return 42\n\
                 }\n\
             }\n\
             return 1\n\
         }\n",
        &[&["internal", "os"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn generic_syscall_write_is_the_native_io_boundary() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.syscall3_value_for_test\n\
         use std/str.str_ptr_addr_for_test\n\
         func main(): i32 {\n\
             let text: &str = \"hello\"\n\
             let written = syscall3_value_for_test(\n\
                 0x02000004,\n\
                 2,\n\
                 str_ptr_addr_for_test(text),\n\
                 5,\n\
             )\n\
             if written == 5 { return 42 }\n\
             return 1\n\
         }\n",
        &[&["internal", "os"], &["str"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_output(&image, 42, b"hello");
}

#[test]
fn generic_syscalls_open_read_and_close_a_native_file() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.{syscall1_fails_for_test, syscall3_value_for_test}\n\
         use std/process.{arg_count_for_test, arg_for_test}\n\
         use std/ptr.{addr, from_ref_mut}\n\
         use std/str.str_ptr_addr_for_test\n\
         func inspect_file(): i32 {\n\
             if !(arg_count_for_test() == 2) { return 2 }\n\
             let fd = syscall3_value_for_test(\n\
                 0x02000005,\n\
                 str_ptr_addr_for_test(arg_for_test(1)),\n\
                 0,\n\
                 0,\n\
             )\n\
             if fd == 18446744073709551615 { return 3 }\n\
             var bytes: [u8; 5] = [0, 0, 0, 0, 0]\n\
             let received = syscall3_value_for_test(\n\
                 0x02000003,\n\
                 fd,\n\
                 addr(from_ref_mut(&+bytes)),\n\
                 5,\n\
             )\n\
             let close_failed = syscall1_fails_for_test(0x02000006, fd)\n\
             if received == 18446744073709551615 { return 4 }\n\
             if !(received == 5) { return 5 }\n\
             if close_failed { return 6 }\n\
             if !(bytes[0] == 104) { return 7 }\n\
             if !(bytes[1] == 101) { return 8 }\n\
             if !(bytes[2] == 108) { return 9 }\n\
             if !(bytes[3] == 108) { return 10 }\n\
             if !(bytes[4] == 111) { return 11 }\n\
             return 42\n\
         }\n\
         func main(): i32 { inspect_file() }\n",
        &[&["internal", "os"], &["process"], &["ptr"], &["str"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_file_read(&image, 42);
}

#[test]
fn process_exit_primitive_crosses_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         func main(): i32 { exit_for_test(42) }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn process_entry_state_crosses_calls_and_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.{arg_count_for_test, arg_for_test, env_count_for_test, env_name_for_test, env_value_for_test}\n\
         use std/str.str_len_for_test\n\
         func inspect_process(): i32 {\n\
             if !(arg_count_for_test() == 2) { return 2 }\n\
             if !(str_len_for_test(arg_for_test(1)) == 6) { return 3 }\n\
             if !(env_count_for_test() == 1) { return 4 }\n\
             if !(str_len_for_test(env_name_for_test(0)) == 1) { return 5 }\n\
             if !(str_len_for_test(env_value_for_test(0)) == 5) { return 6 }\n\
             return 42\n\
         }\n\
         func main(): i32 { inspect_process() }\n",
        &[&["process"], &["str"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_process_state(&image, 42);
}

#[test]
fn trap_and_unreachable_primitives_materialize_without_fallbacks() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/internal/os.terminate_for_test\n\
         func main(): i32 { terminate_for_test(false) }\n",
        &[&["internal", "os"]],
    );
    let machine = lower_machine_fixture(&fixture);
    nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
}

#[test]
fn allocation_abort_primitive_materializes_without_a_runtime_dependency() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/mem.allocation_abort_for_test\n\
         func main(): i32 { allocation_abort_for_test() }\n",
        &[&["mem"]],
    );
    let machine = lower_machine_fixture(&fixture);
    nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
}

#[test]
fn user_destruction_and_drop_flags_cross_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         instance ExitOnDrop {\n\
             pub operator (&self == other: &Self): bool { true }\n\
         }\n\
         func main(): i32 {\n\
             let _ = if false {\n\
                 ExitOnDrop { status: 43 } == ExitOnDrop { status: 44 }\n\
             } else {\n\
                 true\n\
             }\n\
             return 42\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);

    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         instance ExitOnDrop {\n\
             pub operator (&self == other: &Self): bool { true }\n\
         }\n\
         func main(): i32 {\n\
             let _ = if true {\n\
                 ExitOnDrop { status: 43 } == ExitOnDrop { status: 44 }\n\
             } else {\n\
                 true\n\
             }\n\
             return 1\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 44);
}

#[test]
fn compiler_generated_pointer_destruction_crosses_the_native_pipeline() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         struct Container { values: [ExitOnDrop; 2] }\n\
         func main(): i32 {\n\
             var value = Container {\n\
                 values: [ExitOnDrop { status: 41 }, ExitOnDrop { status: 42 }],\n\
             }\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return 1\n\
         }\n",
        &[&["process"], &["ptr"], &["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn copy_pointer_destruction_is_an_explicit_native_noop() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         func main(): i32 {\n\
             var value: i32 = 41\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return value + 1\n\
         }\n",
        &[&["ptr"], &["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    assert_eq!(machine.destructions().iter().len(), 0);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn generated_enum_pointer_destruction_selects_only_the_active_payload() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         enum Choice {\n\
             first(value: ExitOnDrop)\n\
             second(value: ExitOnDrop)\n\
         }\n\
         func main(): i32 {\n\
             var value = Choice.second(ExitOnDrop { status: 42 })\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return 1\n\
         }\n",
        &[&["process"], &["ptr"], &["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn generated_optional_pointer_destruction_selects_present_payload() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         func main(): i32 {\n\
             var value: ExitOnDrop? = ExitOnDrop { status: 42 }\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+value), 0)\n\
             return 1\n\
         }\n",
        &[&["process"], &["ptr"], &["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn generated_closure_pointer_destruction_visits_owned_captures() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process.exit_for_test\n\
         use std/ptr.drop_value_at_ptr_for_test\n\
         use std/ptr.from_ref_mut\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { exit_for_test(self.status) }\n\
         func main(): i32 {\n\
             let resource = ExitOnDrop { status: 42 }\n\
             var callback = (move resource;): void { return }\n\
             drop_value_at_ptr_for_test(from_ref_mut(&+callback), 0)\n\
             return 1\n\
         }\n",
        &[&["process"], &["ptr"], &["ptr"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn checked_dynamic_places_cross_the_native_pipeline() {
    let machine = lower_machine(
        "func main(): i32 {\n\
             var values: [i32; 3] = [20, 1, 2]\n\
             values[1] = 20\n\
             return values[0] + values[1] + values[2]\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn built_in_index_borrows_share_native_address_evaluation() {
    let machine = lower_machine(
        "func read(values: &[i32; 3], index: usize): i32 { values[index] }\n\
         func byte_at(text: &str, index: usize): u8 { text[index] }\n\
         func main(): i32 {\n\
             let values: [i32; 3] = [20, 20, 2]\n\
             if byte_at(\"abc\", 1) == 98 {\n\
                 return read(&values, 0) + read(&values, 1) + read(&values, 2)\n\
             }\n\
             return 1\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_and_assert_status(image: &nocter_macho::MachOImage, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);
    let artifact = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "nocter-native-conformance-{}-{artifact}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path).status().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        status.code(),
        Some(expected),
        "native image terminated with {status:?}"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_and_assert_output(image: &nocter_macho::MachOImage, expected: i32, stderr: &[u8]) {
    use std::os::unix::fs::PermissionsExt;

    let path = std::env::temp_dir().join(format!(
        "nocter-conformance-output-{}-{}",
        std::process::id(),
        expected
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = std::process::Command::new(&path).output().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(output.status.code(), Some(expected));
    assert_eq!(output.stdout, b"");
    assert_eq!(output.stderr, stderr);
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_and_assert_process_state(image: &nocter_macho::MachOImage, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);
    let artifact = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "nocter-process-context-{}-{artifact}",
        std::process::id()
    ));
    std::fs::write(&path, image.bytes()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let status = std::process::Command::new(&path)
        .arg("needle")
        .env_clear()
        .env("N", "value")
        .status()
        .unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(status.code(), Some(expected));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn execute_and_assert_file_read(image: &nocter_macho::MachOImage, expected: i32) {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ARTIFACT: AtomicU64 = AtomicU64::new(0);
    let artifact = NEXT_ARTIFACT.fetch_add(1, Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("nocter-file-io-{}-{artifact}", std::process::id()));
    let executable = base.with_extension("image");
    let input = base.with_extension("input");
    std::fs::write(&executable, image.bytes()).unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&input, b"hello").unwrap();
    let status = std::process::Command::new(&executable)
        .arg(&input)
        .status()
        .unwrap();
    std::fs::remove_file(&input).unwrap();
    std::fs::remove_file(&executable).unwrap();
    assert_eq!(status.code(), Some(expected));
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_and_assert_status(_image: &nocter_macho::MachOImage, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_and_assert_output(_image: &nocter_macho::MachOImage, _expected: i32, _stderr: &[u8]) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_and_assert_process_state(_image: &nocter_macho::MachOImage, _expected: i32) {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn execute_and_assert_file_read(_image: &nocter_macho::MachOImage, _expected: i32) {}

fn lower_machine(source: &str) -> MachineProgram {
    let fixture = CompilerFixture::with_app(source);
    lower_machine_fixture(&fixture)
}

fn lower_test_machine(source: &str) -> MachineProgram {
    let fixture = CompilerFixture::with_tests(source);
    lower_fixture(&fixture, true)
}

fn lower_machine_fixture(fixture: &CompilerFixture) -> MachineProgram {
    lower_fixture(fixture, false)
}

fn lower_fixture(fixture: &CompilerFixture, tests: bool) -> MachineProgram {
    let input = fixture.input();
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (declarations, source_index) = lowered.into_parts();
    let prepared = prepare_program_checking(&input, declarations, source_index).unwrap();
    let checked = check_prepared_program(&input, prepared)
        .unwrap()
        .into_parts()
        .0;
    let standard_package = checked.graph().standard_package().unwrap();
    let snapshot = ToolchainSnapshot::select(
        CompilationTarget::Arm64Darwin,
        standard_package,
        primitive_registry(&checked),
    )
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
    MachineProgram::lower(&lower_executable(executable).unwrap()).unwrap()
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
                let (module, name) = nocter_test_support::primitive_source_location(role);
                (declaration.kind() == CallableKind::Primitive
                    && actual_path == module
                    && declaration
                        .name()
                        .and_then(|name| graph.symbols().spelling(name))
                        == Some(name))
                .then_some(callable)
            })
            .unwrap_or_else(|| panic!("missing fixture primitive {role:?}"));
        PrimitiveBinding::new(role, callable)
    }))
    .unwrap()
}
