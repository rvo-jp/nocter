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
