use nocter_test_support::CompilerFixture;

use super::{execute_and_assert_status, lower_machine, lower_machine_fixture};

#[test]
fn structural_tuples_cross_calls_places_patterns_and_native_execution() {
    let machine = lower_machine(
        "struct Number { value: i32 }\n\
         func make_nested(): (i32, (i32, i32)) { return (1, (20, 21)) }\n\
         func combine(value: (i32, i32)): (i32, i32) {\n\
             var result = value\n\
             result.0 += result.1\n\
             return result\n\
         }\n\
         func main(): i32 {\n\
             let (_, nested) = make_nested()\n\
             let (left, right) = nested\n\
             let result = combine((left, right))\n\
             let owned = (Number { value: 40 }, Number { value: 2 })\n\
             let second = move owned.1\n\
             var borrowed = (result.0, result.1)\n\
             let first_view = &+borrowed.0\n\
             let second_view = &borrowed.1\n\
             if first_view == second_view { return 1 }\n\
             return owned.0.value + second.value + (0, 0).1\n\
         }\n",
    );
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}

#[test]
fn structural_tuple_destruction_runs_in_reverse_position_order() {
    let fixture = CompilerFixture::with_app_standard_uses(
        "use std/process\n\
         struct ExitOnDrop { status: i32 }\n\
         drop ExitOnDrop(&+self) { process.exit_for_test(self.status) }\n\
         func main(): i32 {\n\
             let values = (ExitOnDrop { status: 41 }, ExitOnDrop { status: 42 })\n\
             return 0\n\
         }\n",
        &[&["process"]],
    );
    let machine = lower_machine_fixture(&fixture);
    let program = nocter_arm64::Arm64Program::lower_machine(&machine).unwrap();
    let image = nocter_macho::MachOImage::build(&program).unwrap();

    execute_and_assert_status(&image, 42);
}
