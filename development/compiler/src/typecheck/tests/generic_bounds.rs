use super::check_text;

#[test]
fn enforces_nominal_where_requirements_at_type_use_sites() {
    let diagnostics = check_text(
        r#"interface Marked {}

copy struct MarkedValue {}
conform Marked for MarkedValue {}

struct OwnedValue {}

struct Box<T> where T: Marked {
    value: T
}

struct CopyBox<T> where copy T {
    value: T
}

func generic_valid<T>(value: Box<T>): void where T: Marked {
    return
}

func concrete_valid(value: Box<MarkedValue>, copied: CopyBox<MarkedValue>): void {
    return
}

func invalid(value: Box<OwnedValue>, copied: CopyBox<OwnedValue>): void {
    return
}
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0470")
            .count(),
        2,
        "{diagnostics:?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0470" && diagnostic.message.contains("Marked")
        })
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "E0470" && diagnostic.message.contains("copy T")
        })
    );
}

#[test]
fn enforces_nominal_projection_equalities_at_type_use_sites() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
}

struct IntSource {}
conform Source for IntSource { type Item = i32 }

struct BoolSource {}
conform Source for BoolSource { type Item = bool }

struct Pair<L, R> where L: Source, R: Source, L.Item = R.Item {
    left: L
    right: R
}

func valid(value: Pair<IntSource, IntSource>): void {
    return
}

func invalid(value: Pair<IntSource, BoolSource>): void {
    return
}
"#,
    );

    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "E0471")
            .count(),
        1,
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_copy_requirement_in_generic_body_and_at_copyable_call_site() {
    let diagnostics = check_text(
        r#"func duplicate<T>(value: T): [T; 2] where copy T {
    return [value, value]
}

func main(): i32 {
    let values = duplicate(7)
    return values[0]
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_where_copy_requirement_for_an_enclosing_type_parameter() {
    let diagnostics = check_text(
        r#"struct Pair<T> {
    pub first: T,
    pub second: T,
}

construct Pair<T> {
    pub func duplicate(value: T): Self where copy T {
        return Pair<T> { first: value, second: value }
    }
}

func main(): i32 {
    let pair = Pair.duplicate(7)
    return pair.second
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_non_copy_type_at_copy_required_call() {
    let diagnostics = check_text(
        r#"struct Resource {
    value: i32
}

func duplicate<T>(value: T): [T; 2] where copy T {
    return [value, value]
}

func main(): i32 {
    let resource = Resource { value: 7 }
    let values = duplicate(move resource)
    return values[0].value
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0458"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_unknown_and_duplicate_where_copy_requirements() {
    let diagnostics = check_text(
        r#"func duplicate<T>(value: T): T where copy T, copy Missing, copy T {
    return value
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0452"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0453"),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_builtin_readonly_callable_bound_and_direct_invocation() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F, value: i32): i32 where F: &func(i32): i32 {
    return callback(value)
}

func main(): i32 {
    return invoke((value) { value * 2 }, 7)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_builtin_readwrite_callable_bound_through_writable_place() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F, value: i32): i32 where F: &+func(i32): i32 {
    var callable = move callback
    return callable(value)
}

func main(): i32 {
    var total: i32 = 0
    return invoke((&+total; value) {
        total += value
        total
    }, 7)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_readwrite_callable_invocation_through_immutable_place() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F, value: i32): i32 where F: &+func(i32): i32 {
    return callback(value)
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0454"),
        "{diagnostics:?}"
    );
}

#[test]
fn consuming_callable_invocation_moves_the_callback() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F, value: i32): i32 where F: func(i32): i32 {
    let first = callback(value)
    return callback(first)
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("after it was moved")
                || diagnostic.message.contains("already moved")
                || diagnostic.message.contains("because it was moved")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_multiple_callable_contracts_on_one_parameter() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F): i32 where F: &func(i32): i32 + func(i32): i32 {
    return 0
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0455"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_invalid_callable_parameter_and_provenance_names() {
    let diagnostics = check_text(
        r#"func invoke<F>(callback: F): i32 where F: &func(value: &i32, value: &i32): &i32 from missing {
    return 0
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0456"),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0457"),
        "{diagnostics:?}"
    );
}

const MEASURE_INTERFACE: &str = r#"interface Measure {
    pub method &self.measure(): i32
}

struct Count {
    value: i32
}

"#;

#[test]
fn accepts_statically_resolved_method_call_through_interface_bound() {
    let diagnostics = check_text(&format!(
        r#"{MEASURE_INTERFACE}
conform Measure for Count {{
    method &self.measure(): i32 {{
        return self.value
    }}
}}

func read<T>(value: &T): i32 where T: Measure {{
    return value.measure()
}}

func main(): i32 {{
    let value = Count {{ value: 7 }}
    return read(&value)
}}
"#
    ));

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_statically_resolved_method_local_generics_through_an_interface_bound() {
    let diagnostics = check_text(
        r#"interface Identity {
    pub method &self.apply<T>(value: T): T from value
}

struct IdentityValue {
    marker: i32
}

conform Identity for IdentityValue {
    method &self.apply<U>(value: U): U from value {
        return value
    }
}

func apply_i32<I>(identity: &I, value: i32): i32 where I: Identity {
    return identity.apply(value)
}

func main(): i32 {
    let identity = IdentityValue { marker: 0 }
    return apply_i32(&identity, 7)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_concrete_type_that_does_not_satisfy_bound() {
    let diagnostics = check_text(&format!(
        r#"{MEASURE_INTERFACE}
func read<T>(value: &T): i32 where T: Measure {{
    return value.measure()
}}

func main(): i32 {{
    let value = Count {{ value: 7 }}
    return read(&value)
}}
"#
    ));

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0447")
    );
}

#[test]
fn diagnoses_non_interface_generic_bound() {
    let diagnostics = check_text(
        r#"struct Value {
    raw: i32
}

func identity<T>(value: T): T where T: Value {
    return value
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0446")
    );
}

#[test]
fn accepts_forwarding_a_matching_generic_bound() {
    let diagnostics = check_text(&format!(
        r#"{MEASURE_INTERFACE}
conform Measure for Count {{
    method &self.measure(): i32 {{
        return self.value
    }}
}}

func read<T>(value: &T): i32 where T: Measure {{
    return value.measure()
}}

func forward<U>(value: &U): i32 where U: Measure {{
    return read(value)
}}

func main(): i32 {{
    return 0
}}
"#
    ));

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn preserves_receiver_provenance_through_generic_bound_call() {
    let diagnostics = check_text(
        r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

struct Box<T> {
    value: T
}

conform Lookup<T> for Box<T> {
    method &self.get(): &T from self {
        return &self.value
    }
}

func lookup<M, V>(map: &M): &V from map where M: Lookup<V> {
    return map.get()
}

func inspect(value: &i32): void {
    return
}

func main(): i32 {
    var box = Box<i32> { value: 7 }
    let found = lookup(&box)
    box = Box<i32> { value: 8 }
    inspect(found)
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0434"),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_conformance_body_with_elided_unique_provenance_contract() {
    let diagnostics = check_text(
        r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

struct Box<T> {
    value: T
}

conform Lookup<T> for Box<T> {
    method &self.get(): &T {
        return &self.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_multiple_independent_interface_bounds() {
    let diagnostics = check_text(
        r#"interface Read {
    pub method &self.read(): i32
}

interface Size {
    pub method &self.size(): usize
}

struct Value {
    raw: i32
}

conform Read for Value {
    method &self.read(): i32 {
        return self.raw
    }
}

conform Size for Value {
    method &self.size(): usize {
        return 1
    }
}

func inspect<T>(value: &T): i32 where T: Read + Size {
    let size: usize = value.size()
    return value.read()
}

func main(): i32 {
    let value = Value { raw: 9 }
    return inspect(&value)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_duplicate_interface_bound_by_specialized_identity() {
    let diagnostics = check_text(
        r#"interface Read<T> {
    pub method &self.read(): T
}

func inspect<T>(value: &T): i32 where T: Read<i32> + Read<i32> {
    return value.read()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0450"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_ambiguous_method_from_distinct_bounds() {
    let diagnostics = check_text(
        r#"interface Left {
    pub method &self.read(): i32
}

interface Right {
    pub method &self.read(): i32
}

func inspect<T>(value: &T): i32 where T: Left + Right {
    return value.read()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0449"),
        "{diagnostics:?}"
    );
}

#[test]
fn conditional_conformance_requires_its_generic_bound() {
    let accepted = check_text(
        r#"interface Source<T> {
    pub method &+self.next(): T?
}

interface Wrapper<T> {
    pub method &+self.next(): T?
}

struct Input {
    value: i32
}

conform Source<T> for Input where T = i32 {
    method &+self.next(): i32? {
        return none
    }
}

struct Adapter<T, I> {
    input: I
}

conform Wrapper<T> for Adapter<T, I> where I: Source<T> {
    method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

func use_wrapper<W>(wrapper: &+W): void where W: Wrapper<i32> {
    let item = wrapper.next()
    return
}

func main(): i32 {
    var adapter = Adapter<i32, Input> { input: Input { value: 1 } }
    use_wrapper(&+adapter)
    return 0
}
"#,
    );
    assert!(accepted.is_empty(), "{accepted:?}");

    let rejected = check_text(
        r#"interface Source<T> {
    pub method &+self.next(): T?
}

interface Wrapper<T> {
    pub method &+self.next(): T?
}

struct Input {
    value: i32
}

struct Adapter<T, I> {
    input: I
}

conform Wrapper<T> for Adapter<T, I> where I: Source<T> {
    method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

func use_wrapper<W>(wrapper: &+W): void where W: Wrapper<i32> {
    let item = wrapper.next()
    return
}

func main(): i32 {
    var adapter = Adapter<i32, Input> { input: Input { value: 1 } }
    use_wrapper(&+adapter)
    return 0
}
"#,
    );
    assert!(
        rejected.iter().any(|diagnostic| diagnostic.code == "E0447"),
        "{rejected:?}"
    );
}
