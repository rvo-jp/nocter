use super::check_text;

#[test]
fn accepts_builtin_readonly_callable_bound_and_direct_invocation() {
    let diagnostics = check_text(
        r#"func invoke<F: &func(i32): i32>(callback: F, value: i32): i32 {
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
        r#"func invoke<F: &+func(i32): i32>(callback: F, value: i32): i32 {
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
        r#"func invoke<F: &+func(i32): i32>(callback: F, value: i32): i32 {
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
        r#"func invoke<F: func(i32): i32>(callback: F, value: i32): i32 {
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
        r#"func invoke<F: &func(i32): i32 + func(i32): i32>(callback: F): i32 {
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
        r#"func invoke<F: &func(value: &i32, value: &i32): &i32 from missing>(callback: F): i32 {
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
impl Measure for Count {{
    method &self.measure(): i32 {{
        return self.value
    }}
}}

func read<T: Measure>(value: &T): i32 {{
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
    pub method &self.apply<T>(value: T): T
}

struct IdentityValue {
    marker: i32
}

impl Identity for IdentityValue {
    method &self.apply<U>(value: U): U {
        return value
    }
}

func apply_i32<I: Identity>(identity: &I, value: i32): i32 {
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
func read<T: Measure>(value: &T): i32 {{
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

func identity<T: Value>(value: T): T {
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
impl Measure for Count {{
    method &self.measure(): i32 {{
        return self.value
    }}
}}

func read<T: Measure>(value: &T): i32 {{
    return value.measure()
}}

func forward<U: Measure>(value: &U): i32 {{
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

impl<T> Lookup<T> for Box<T> {
    method &self.get(): &T from self {
        return &self.value
    }
}

func lookup<M: Lookup<V>, V>(map: &M): &V from map {
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
fn diagnoses_conformance_with_missing_public_provenance_contract() {
    let diagnostics = check_text(
        r#"interface Lookup<V> {
    pub method &self.get(): &V from self
}

struct Box<T> {
    value: T
}

impl<T> Lookup<T> for Box<T> {
    method &self.get(): &T {
        return &self.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0426"),
        "{diagnostics:?}"
    );
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

impl Read for Value {
    method &self.read(): i32 {
        return self.raw
    }
}

impl Size for Value {
    method &self.size(): usize {
        return 1
    }
}

func inspect<T: Read + Size>(value: &T): i32 {
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

func inspect<T: Read<i32> + Read<i32>>(value: &T): i32 {
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

func inspect<T: Left + Right>(value: &T): i32 {
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

impl Source<i32> for Input {
    method &+self.next(): i32? {
        return none
    }
}

struct Adapter<T, I> {
    input: I
}

impl<T, I: Source<T>> Wrapper<T> for Adapter<T, I> {
    method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

func use_wrapper<W: Wrapper<i32>>(wrapper: &+W): void {
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

impl<T, I: Source<T>> Wrapper<T> for Adapter<T, I> {
    method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

func use_wrapper<W: Wrapper<i32>>(wrapper: &+W): void {
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
