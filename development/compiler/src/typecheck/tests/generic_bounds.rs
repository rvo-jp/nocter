use super::check_text;

const MEASURE_INTERFACE: &str = r#"interface Measure {
    pub method &self.measure(): i32
}

struct Count {
    value: i32
}

impl Count {
    pub method &self.measure(): i32 {
        return self.value
    }
}
"#;

#[test]
fn accepts_statically_resolved_method_call_through_interface_bound() {
    let diagnostics = check_text(&format!(
        r#"{MEASURE_INTERFACE}
impl Measure for Count

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
impl Measure for Count

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

impl<U> Box<U> {
    pub method &self.get(): &U from self {
        return &self.value
    }
}

impl<T> Lookup<T> for Box<T>

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

impl<U> Box<U> {
    pub method &self.get(): &U {
        return &self.value
    }
}

impl<T> Lookup<T> for Box<T>

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

impl Value {
    pub method &self.read(): i32 {
        return self.raw
    }

    pub method &self.size(): usize {
        return 1
    }
}

impl Read for Value
impl Size for Value

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

impl Input {
    pub method &+self.next(): i32? {
        return none
    }
}

impl Source<i32> for Input

struct Adapter<T, I> {
    input: I
}

impl<T, I: Source<T>> Adapter<T, I> {
    pub method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

impl<T, I: Source<T>> Wrapper<T> for Adapter<T, I>

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

impl Input {
    pub method &+self.next(): i32? {
        return none
    }
}

struct Adapter<T, I> {
    input: I
}

impl<T, I: Source<T>> Adapter<T, I> {
    pub method &+self.next(): T? {
        return self.input.next() otherwise { return none }
    }
}

impl<T, I: Source<T>> Wrapper<T> for Adapter<T, I>

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
