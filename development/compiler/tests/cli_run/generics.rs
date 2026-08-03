use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_function_exit_code() {
    let project = TempProject::new("cli-run-generic-function");
    let source = project.write_source(
        "generic_function.nct",
        r#"func identity<T>(value: T): T {
    return value
}

func main(): i32 {
    return identity(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_associated_function_exit_code() {
    let project = TempProject::new("cli-run-generic-associated-function");
    let source = project.write_source(
        "generic_associated_function.nct",
        r#"struct Box<T> {
    value: T
}

func Box.unwrap<T>(box: Box<T>): T {
    return box.value
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return Box.unwrap(move box)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nested_generic_function_exit_code() {
    let project = TempProject::new("cli-run-nested-generic-function");
    let source = project.write_source(
        "nested_generic_function.nct",
        r#"func identity<T>(value: T): T {
    return value
}

func forward<T>(value: T): T {
    return identity(value)
}

func main(): i32 {
    return forward(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_function_inferred_from_binding_exit_code() {
    let project = TempProject::new("cli-run-generic-function-expected-binding");
    let source = project.write_source(
        "generic_function_expected_binding.nct",
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func main(): i32 {
    let marker: Marker<u8> = make()
    return marker.code
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_function_inferred_from_parameter_exit_code() {
    let project = TempProject::new("cli-run-generic-function-expected-parameter");
    let source = project.write_source(
        "generic_function_expected_parameter.nct",
        r#"struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func main(): i32 {
    return consume(make())
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_nested_generic_function_inferred_from_parameter_exit_code() {
    let project = TempProject::new("cli-run-nested-generic-function-expected-parameter");
    let source = project.write_source(
        "nested_generic_function_expected_parameter.nct",
        r#"copy struct Marker<T> {
    code: i32
}

func make<T>(): Marker<T> {
    return Marker<T> { code: 42 }
}

func forward<T>(value: T): T {
    return value
}

func consume(marker: Marker<u8>): i32 {
    return marker.code
}

func main(): i32 {
    return consume(forward(make()))
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_impl_method_body_generic_function_exit_code() {
    let project = TempProject::new("cli-run-generic-method-body-function");
    let source = project.write_source(
        "generic_method_body_function.nct",
        r#"struct Box<T> {
    value: T
}

func identity<T>(value: T): T {
    return value
}

impl<U> Box<U> {
    method self.into_identity(): U {
        return identity(self.value)
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return (move box).into_identity()
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_function_body_method_call_exit_code() {
    let project = TempProject::new("cli-run-generic-function-body-method");
    let source = project.write_source(
        "generic_function_body_method.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func forward<T>(box: Box<T>): T {
    return (move box).into_value()
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return forward(move box)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_concrete_generic_impl_method_exit_code() {
    let project = TempProject::new("cli-run-concrete-generic-impl-method");
    let source = project.write_source(
        "concrete_generic_impl_method.nct",
        r#"struct Box<T> {
    value: T
}

impl Box<i32> {
    method &self.read(): i32 {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return box.read()
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_impl_method_with_concrete_receiver_exit_code() {
    let project = TempProject::new("cli-run-generic-impl-method");
    let source = project.write_source(
        "generic_impl_method.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let box = Box<i32> { value: 42 }
    return (move box).into_value()
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_returns_generic_impl_method_multiple_concrete_receivers_exit_code() {
    let project = TempProject::new("cli-run-generic-impl-method-multiple");
    let source = project.write_source(
        "generic_impl_method_multiple.nct",
        r#"struct Box<T> {
    value: T
}

impl<U> Box<U> {
    method self.into_value(): U {
        return self.value
    }
}

func main(): i32 {
    let first_box = Box<i32> { value: 42 }
    let second_box = Box<u8> { value: 7 }
    let first = (move first_box).into_value()
    let second = (move second_box).into_value()
    return first + (second as i32)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(49),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_specializes_multiple_bounds_and_conditional_conformance() {
    let project = TempProject::new("cli-run-conditional-conformance");
    let source = project.write_source(
        "conditional_conformance.nct",
        r#"interface Read {
    pub method &self.read(): i32
}

interface Size {
    pub method &self.size(): i32
}

struct Value {
    value: i32
}

impl Value {
    pub method &self.read(): i32 {
        return self.value
    }

    pub method &self.size(): i32 {
        return 1
    }
}

impl Read for Value
impl Size for Value

struct Adapter<T> {
    value: T
}

impl<T: Read> Adapter<T> {
    pub method &self.read(): i32 {
        return self.value.read()
    }
}

impl<T: Read> Read for Adapter<T>

func inspect<T: Read + Size>(value: &T): i32 {
    return value.read() + value.size()
}

func read<T: Read>(value: &T): i32 {
    return value.read()
}

func main(): i32 {
    let value = Value { value: 41 }
    let inspected: i32 = inspect(&value)
    let adapter = Adapter<Value> { value: Value { value: inspected } }
    return read(&adapter)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_invokes_interface_default_method() {
    let project = TempProject::new("cli-run-interface-default-method");
    let source = project.write_source(
        "interface_default_method.nct",
        r#"interface Value {
    pub method &self.value(): i32 {
        return 42
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_interface_default_method_calls_required_method() {
    let project = TempProject::new("cli-run-interface-default-calls-required");
    let source = project.write_source(
        "interface_default_calls_required.nct",
        r#"interface Value {
    pub method &self.value(): i32

    pub method &self.incremented(): i32 {
        let value = self.value()
        return value + 1
    }
}

copy struct Number {
    value: i32
}

impl Number {
    pub method &self.value(): i32 {
        return self.value
    }
}

impl Value for Number

func main(): i32 {
    let number = Number { value: 41 }
    return number.incremented()
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_specializes_interface_default_method_through_generic_bound() {
    let project = TempProject::new("cli-run-interface-default-generic-bound");
    let source = project.write_source(
        "interface_default_generic_bound.nct",
        r#"interface Value {
    pub method &self.value(): i32 {
        return 42
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit

func read<T: Value>(value: &T): i32 {
    return value.value()
}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return read(&unit)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_generic_bound_dispatches_to_inherent_default_override() {
    let project = TempProject::new("cli-run-interface-default-override");
    let source = project.write_source(
        "interface_default_override.nct",
        r#"interface Value {
    pub method &self.value(): i32 {
        return 1
    }
}

copy struct Unit {
    marker: i32
}

impl Unit {
    pub method &self.value(): i32 {
        return 42
    }
}

impl Value for Unit

func read<T: Value>(value: &T): i32 {
    return value.value()
}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return read(&unit)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_specializes_generic_interface_default_method() {
    let project = TempProject::new("cli-run-generic-interface-default-method");
    let source = project.write_source(
        "generic_interface_default_method.nct",
        r#"interface Identity {
    pub method &self.identity<T>(value: T): T {
        return value
    }
}

copy struct Unit {
    marker: i32
}

impl Identity for Unit

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.identity(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn run_command_specializes_generic_interface_owner_default_method() {
    let project = TempProject::new("cli-run-generic-interface-owner-default");
    let source = project.write_source(
        "generic_interface_owner_default.nct",
        r#"interface Value<T> {
    pub method &self.value(fallback: T): T {
        return fallback
    }
}

copy struct Unit {
    marker: i32
}

impl Value<i32> for Unit

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value(42)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
