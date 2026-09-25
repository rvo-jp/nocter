use nocter_declaration_lowering::lower_compile_unit_declarations;
use nocter_syntax::NodeKind;
use nocter_toolchain_contract::StandardDeclarationRole;

use super::check_prepared_program;
use crate::test_support::{Fixture, StandardRoleInput, with_standard_roles};
use crate::{BodyRule, prepare_program_checking};

fn check(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::new(source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

fn check_standard(source: &str) -> Result<crate::CheckedProgramOutput, crate::BodyCheckError> {
    let fixture = Fixture::with_standard("", source);
    let input = fixture.input(false);
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared)
}

const TEXT_DECLARATIONS: &str = r#"
struct Text {}
construct Text {
    pub literal ""(text: &str): Self { return Self {} }
}
"#;

#[test]
fn direct_allocation_violates_noalloc() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nnoalloc func invalid(): Text {{ return Text \"value\" }}\n"
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0411");
}

#[test]
fn creating_a_deferred_computation_is_an_allocation_fact() {
    let error = check(
        "async func produce(): i32 { 1 }\n\
         noalloc func invalid(): (future i32)? { produce() }\n",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn allocation_facts_propagate_through_source_backed_calls() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nfunc allocate(): Text {{ return Text \"value\" }}\nnoalloc func invalid(): Text {{ return allocate() }}\n"
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn source_backed_unmarked_helpers_can_be_proven_allocation_free() {
    check(
        "func helper(value: i32): i32 { return value + 1 }\nnoalloc func valid(value: i32): i32 { return helper(value) }\n",
    )
    .unwrap();
}

#[test]
fn notrap_rejects_source_semantic_trap_operations() {
    for source in [
        "notrap func add(left: i32, right: i32): i32 { return left + right }\n",
        "notrap func force(value: i32?): i32 { return value! }\n",
        "notrap func index(values: &[i32], position: usize): i32 { return values[position] }\n",
        "notrap func assign(value: i32): i32 { var result = value\nresult += 1\nreturn result }\n",
    ] {
        let error = check(source).unwrap_err();
        assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0425");
    }
}

#[test]
fn notrap_accepts_operations_without_a_source_trap_path() {
    check(
        "func identity(value: i32): i32 { return value }\n\
         notrap func valid(value: i32): i32 { return identity(value) }\n\
         notrap func floating(left: f64, right: f64): f64 { return left / right }\n\
         notrap func propagate(value: i32!): i32! { return move value? }\n",
    )
    .unwrap();
}

#[test]
fn notrap_uses_the_frozen_fixed_array_bounds_disposition() {
    check("notrap func valid(values: [i32; 2]): i32 { return values[1] }\n").unwrap();

    let error =
        check("notrap func invalid(values: [i32; 2]): i32 { return values[2] }\n").unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));
}

#[test]
fn notrap_uses_branch_local_bounds_proof_and_terminal_path_refinement() {
    check(
        "notrap func guarded(values: [i32; 2], index: usize): i32 {\n\
             if index < 2 { return values[index] }\n\
             return 0\n\
         }\n\
         notrap func guarded_by_exit(values: [i32; 2], index: usize): i32 {\n\
             if index >= 2 { return 0 }\n\
             return values[index]\n\
         }\n\
         notrap func guarded_by_inclusive_bound(values: [i32; 2], index: usize): i32 {\n\
             if index <= 1 { return values[index] }\n\
             return 0\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn bounds_proof_does_not_escape_a_join_or_attach_to_mutable_storage() {
    let joined = check(
        "notrap func invalid(values: [i32; 2], index: usize, choose: bool): i32 {\n\
             if choose {\n\
                 if index < 2 { let observed = values[index] }\n\
             }\n\
             return values[index]\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(joined.rule(), Some(BodyRule::NoTrapContractViolation));

    let mutable = check(
        "notrap func invalid(values: [i32; 2], input: usize): i32 {\n\
             var index = input\n\
             if index < 2 { return values[index] }\n\
             return 0\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(mutable.rule(), Some(BodyRule::NoTrapContractViolation));

    let loop_exit = check(
        "notrap func invalid(values: [i32; 2], index: usize, enter: bool): i32 {\n\
             while enter {\n\
                 if index >= 2 { return 0 }\n\
                 break\n\
             }\n\
             return values[index]\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(loop_exit.rule(), Some(BodyRule::NoTrapContractViolation));
}

#[test]
fn trap_facts_propagate_through_calls_and_structural_contracts() {
    let error = check(
        "func increment(value: i32): i32 { return value + 1 }\n\
         notrap func invalid(value: i32): i32 { return increment(value) }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));

    check(
        "notrap func apply(callback: any notrap &func(i32): i32, value: i32): i32 {\n\
             return callback(value)\n\
         }\n",
    )
    .unwrap();

    let error = check(
        "notrap func invalid(callback: any &func(i32): i32, value: i32): i32 {\n\
             return callback(value)\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));
}

#[test]
fn contextual_notrap_closure_requirements_validate_the_closure_body() {
    check(
        "func accept(callback: any notrap &func(i32): i32): i32 { return callback(1) }\n\
         func valid(): i32 { return accept((value: i32): i32 { return value }) }\n",
    )
    .unwrap();

    let error = check(
        "func accept(callback: any notrap &func(i32): i32): i32 { return callback(1) }\n\
         func invalid(): i32 { return accept((value: i32): i32 { return value + 1 }) }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));
}

#[test]
fn trap_facts_include_implicit_destruction() {
    let error = check(
        "struct Resource { value: i32 }\n\
         drop Resource(&+self) { let _ = self.value + 1\nreturn }\n\
         notrap func invalid(value: i32): void { let resource = Resource { value: value }\nreturn }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));

    check(
        "struct Resource {}\n\
         notrap drop Resource(&+self) { return }\n\
         notrap func valid(): void { let resource = Resource {}\nreturn }\n",
    )
    .unwrap();
}

#[test]
fn explicit_process_termination_is_not_a_source_trap() {
    let fixture = Fixture::with_standard(
        "",
        "pub notrap func abort(): never { abort_raw() }\n\
         pub notrap func exit(code: i32): never { exit_raw(code) }\n\
         notrap primitive func abort_raw(): never\n\
         notrap primitive func exit_raw(code: i32): never\n\
         notrap func stop_now(): never { abort() }\n\
         notrap func stop_with(code: i32): never { exit(code) }\n",
    );
    let input = with_standard_roles(
        fixture.input(false),
        vec![
            StandardRoleInput::new(
                StandardDeclarationRole::ProcessAbort,
                fixture.standard_declaration_token(NodeKind::FunctionDeclaration, "abort"),
            ),
            StandardRoleInput::new(
                StandardDeclarationRole::ProcessExit,
                fixture.standard_declaration_token(NodeKind::FunctionDeclaration, "exit"),
            ),
        ],
    );
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    check_prepared_program(&input, prepared).unwrap();
}

#[test]
fn process_termination_roles_do_not_hide_prior_safety_traps() {
    let fixture = Fixture::with_standard(
        "",
        "pub notrap func abort(): never { let _ = 1 + 1\nabort_raw() }\n\
         notrap primitive func abort_raw(): never\n",
    );
    let input = with_standard_roles(
        fixture.input(false),
        vec![StandardRoleInput::new(
            StandardDeclarationRole::ProcessAbort,
            fixture.standard_declaration_token(NodeKind::FunctionDeclaration, "abort"),
        )],
    );
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoTrapContractViolation));
}

#[test]
fn structural_operation_requirements_carry_notrap_into_generic_bodies() {
    check(
        "struct Value { field: i32 }\n\
         instance Value {\n\
             pub noalloc notrap operator (&self == other: &Self): bool { return true }\n\
             pub noalloc notrap operator (&self[index: usize]): &i32 { return &self.field }\n\
             pub noalloc notrap coerce &self as &i32 { return &self.field }\n\
         }\n\
         noalloc notrap func equal<T>(left: &T, right: &T): bool where noalloc notrap (&T == &T): bool {\n\
             return left == right\n\
         }\n\
         noalloc notrap func indexed<C, V>(container: &C, index: usize): &V where noalloc notrap (&C[usize]): &V {\n\
             return &container[index]\n\
         }\n\
         noalloc notrap func converted<T, V>(value: &T): &V where noalloc notrap &T as &V { return value }\n\
         notrap func equal_with_weaker_requirement<T>(left: &T, right: &T): bool where notrap (&T == &T): bool {\n\
             return left == right\n\
         }\n\
         notrap func call_with_stronger_fact<T>(left: &T, right: &T): bool where noalloc notrap (&T == &T): bool {\n\
             return equal_with_weaker_requirement(left, right)\n\
         }\n\
         func use_all(left: &Value, right: &Value): bool {\n\
             let same = equal(left, right)\n\
             let selected: &i32 = indexed(left, 0)\n\
             let converted_view: &i32 = converted(left)\n\
             return same\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn concrete_structural_implementations_must_satisfy_required_guarantees() {
    assert!(
        check(
            "struct Value {}\n\
             instance Value { pub operator (&self == other: &Self): bool { return true } }\n\
             func equal<T>(left: &T, right: &T): bool where notrap (&T == &T): bool {\n\
                 return left == right\n\
             }\n\
             func invalid(left: &Value, right: &Value): bool { return equal(left, right) }\n",
        )
        .is_err()
    );
}

#[test]
fn interface_prerequisites_combine_independent_structural_guarantees() {
    check(
        "pub interface NoAllocEqual where noalloc (&Self == &Self): bool {}\n\
         pub interface NoTrapEqual where notrap (&Self == &Self): bool {}\n\
         pub interface SafeEqual where Self impl NoAllocEqual, Self impl NoTrapEqual {}\n\
         struct Value {}\n\
         instance Value {\n\
             impl NoAllocEqual\n\
             impl NoTrapEqual\n\
             impl SafeEqual\n\
             pub noalloc notrap operator (&self == other: &Self): bool { return true }\n\
         }\n\
         noalloc notrap func equal<T>(left: &T, right: &T): bool where T impl SafeEqual {\n\
             return left == right\n\
         }\n\
         func concrete(left: &Value, right: &Value): bool { return equal(left, right) }\n",
    )
    .unwrap();
}

#[test]
fn synchronous_wait_facts_propagate_through_the_existing_call_graph() {
    let error = check_standard(
        "blocking primitive func wait_raw(): void\n\
         blocking func helper(): void { wait_raw() }\n\
         func invalid(): void { helper() }\n",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::BlockingContractViolation));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0417");

    check_standard(
        "blocking primitive func wait_raw(): void\n\
         blocking func helper(): void { wait_raw() }\n\
         blocking func valid(): void { helper() }\n",
    )
    .unwrap();
}

#[test]
fn authored_blocking_contract_is_not_weakened_by_a_nonwaiting_body() {
    let error = check(
        "blocking func admitted_wait(): void { return }\n\
         async func invalid(): void { admitted_wait() }\n",
    )
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::BlockingContractViolation));
    assert_eq!(error.source_diagnostic().unwrap().code(), "E0417");
}

#[test]
fn asynchronous_bodies_and_destruction_are_always_nonblocking() {
    let error = check_standard(
        "blocking primitive func wait_raw(): void\n\
         async func invalid(): void { wait_raw() }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::BlockingContractViolation));

    let error = check_standard(
        "blocking primitive func wait_raw(): void\n\
         struct Resource {}\n\
         drop Resource(&+self) { wait_raw() }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::BlockingContractViolation));
}

#[test]
fn closure_blocking_is_checked_against_its_structural_contract() {
    check_standard(
        "blocking primitive func wait_raw(): void\n\
         blocking func valid(): void {\n\
             let callback: blocking func(): void = () { wait_raw() }\n\
             callback()\n\
         }\n",
    )
    .unwrap();

    let error = check_standard(
        "blocking primitive func wait_raw(): void\n\
         func invalid(): void {\n\
             let callback: func(): void = () { wait_raw() }\n\
             callback()\n\
         }\n",
    )
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::BlockingContractViolation));
}

#[test]
fn allocation_free_mutual_recursion_reaches_the_least_fixed_point() {
    check(
        "noalloc func even(value: i32): bool {\n    if value == 0 { return true }\n    return odd(value - 1)\n}\nnoalloc func odd(value: i32): bool {\n    if value == 0 { return false }\n    return even(value - 1)\n}\n",
    )
    .unwrap();
}

#[test]
fn allocating_recursive_group_is_independent_of_declaration_order() {
    let first = format!(
        "{TEXT_DECLARATIONS}\n\
         func even(value: i32): Text {{\n\
             if value == 0 {{ return Text \"done\" }}\n\
             return odd(value - 1)\n\
         }}\n\
         func odd(value: i32): Text {{ return even(value - 1) }}\n\
         noalloc func invalid(value: i32): Text {{ return odd(value) }}\n"
    );
    let second = format!(
        "{TEXT_DECLARATIONS}\n\
         noalloc func invalid(value: i32): Text {{ return odd(value) }}\n\
         func odd(value: i32): Text {{ return even(value - 1) }}\n\
         func even(value: i32): Text {{\n\
             if value == 0 {{ return Text \"done\" }}\n\
             return odd(value - 1)\n\
         }}\n"
    );

    for source in [&first, &second] {
        let error = check(source).unwrap_err();
        assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
        assert_eq!(error.source_diagnostic().unwrap().code(), "E0411");
    }
}

#[test]
fn invoked_closure_facts_are_distinct_from_closure_creation() {
    check(
        "noalloc func valid(value: i32): i32 {\n    let callback: noalloc func(i32): i32 = (item) { item + 1 }\n    return callback(value)\n}\n",
    )
    .unwrap();

    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nnoalloc func invalid(): Text {{\n    let callback: noalloc func(): Text = () {{ Text \"value\" }}\n    return callback()\n}}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn implicit_destruction_participates_in_the_same_execution_graph() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nstruct Owned {{}}\ndrop Owned(&+self) {{ let _ = Text \"drop\"\n return }}\nnoalloc func invalid(): void {{\n    let value = Owned {{}}\n    return\n}}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));

    let error = check(&format!(
        "{TEXT_DECLARATIONS}\nstruct Owned {{}}\nnoalloc drop Owned(&+self) {{ let _ = Text \"drop\"\n return }}\n"
    ))
    .unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn concrete_generic_aggregate_uses_its_substituted_destruction_dependencies() {
    check(
        "struct Owned {}\n\
         noalloc drop Owned(&+self) { return }\n\
         struct Wrapper<T> { value: T }\n\
         noalloc func consume(value: Wrapper<Owned>): void { return }\n",
    )
    .unwrap();
}

#[test]
fn opaque_cleanup_uses_its_selected_witness_destruction_dependencies() {
    check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Value {}\n\
         noalloc drop Value(&+self) { return }\n\
         instance Value {\n\
             impl Show\n\
             noalloc method &self.show(): i32 { return 1 }\n\
         }\n\
         noalloc func make(): some Show { return Value {} }\n\
         noalloc func consume(): void {\n\
             let value = make()\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn generic_opaque_cleanup_substitutes_witness_arguments() {
    check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Value {}\n\
         noalloc drop Value(&+self) { return }\n\
         struct Wrapper<T> { value: T }\n\
         instance Wrapper<T> {\n\
             impl Show\n\
             noalloc method &self.show(): i32 { return 1 }\n\
         }\n\
         noalloc func make<T>(value: T): some Show {\n\
             return Wrapper { value: move value }\n\
         }\n\
         noalloc func consume(value: Value): void {\n\
             let hidden = make(move value)\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn optional_opaque_cleanup_reuses_the_same_witness_authority() {
    check(
        "pub interface Show { pub method &self.show(): i32 }\n\
         struct Value {}\n\
         noalloc drop Value(&+self) { return }\n\
         instance Value {\n\
             impl Show\n\
             noalloc method &self.show(): i32 { return 1 }\n\
         }\n\
         noalloc func make(present: bool): some Show? {\n\
             if present { return Value {} }\n\
             return none\n\
         }\n\
         noalloc func consume(present: bool): void {\n\
             let value = make(present)\n\
             return\n\
         }\n",
    )
    .unwrap();
}

#[test]
fn opaque_cleanup_retains_an_allocating_witness_drop_edge() {
    let error = check(&format!(
        "{TEXT_DECLARATIONS}\n\
         pub interface Show {{ pub method &self.show(): i32 }}\n\
         struct Value {{}}\n\
         drop Value(&+self) {{ let _ = Text \"drop\"\n return }}\n\
         instance Value {{\n\
             impl Show\n\
             noalloc method &self.show(): i32 {{ return 1 }}\n\
         }}\n\
         noalloc func make(): some Show {{ return Value {{}} }}\n\
         noalloc func consume(): void {{\n\
             let value = make()\n\
             return\n\
         }}\n"
    ))
    .unwrap_err();

    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}

#[test]
fn enum_residual_dependencies_exclude_the_transferred_payload() {
    check(&format!(
        "{TEXT_DECLARATIONS}\n\
         struct Transferred {{}}\n\
         drop Transferred(&+self) {{ let _ = Text \"drop\"\n return }}\n\
         struct Retained {{}}\n\
         noalloc drop Retained(&+self) {{ return }}\n\
         enum Pair {{ values(first: Transferred, second: Retained) }}\n\
         noalloc drop Pair(&+self) {{ return }}\n\
         noalloc func take(first: Transferred, second: Retained): Transferred {{\n\
             return match Pair.values(move first, move second) {{\n\
                 Pair.values(item, _) {{ move item }}\n\
             }}\n\
         }}\n"
    ))
    .unwrap();
}

#[test]
fn compiler_selected_allocation_request_is_a_positive_fact_seed() {
    let fixture = Fixture::with_standard(
        "",
        "pub func request(size: usize): usize { return size }\nnoalloc func invalid(): usize { return request(1) }\n",
    );
    let input = with_standard_roles(
        fixture.input(false),
        vec![StandardRoleInput::new(
            StandardDeclarationRole::AllocationRequest,
            fixture.standard_declaration_token(NodeKind::FunctionDeclaration, "request"),
        )],
    );
    let lowered = lower_compile_unit_declarations(&input).unwrap();
    let (program, frontend_bindings, source_index) = lowered.into_checking_parts();
    let prepared =
        prepare_program_checking(&input, program, &frontend_bindings, source_index).unwrap();
    let error = check_prepared_program(&input, prepared).unwrap_err();
    assert_eq!(error.rule(), Some(BodyRule::NoAllocationContractViolation));
}
