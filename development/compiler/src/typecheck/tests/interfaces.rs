use super::check_text;

#[test]
fn accepts_interface_default_without_inherent_implementation() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32 {
        return 7
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn default_body_can_call_required_method() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32

    pub method &self.doubled(): i32 {
        return self.value() * 2
    }
}

copy struct Number {
    value: i32
}

impl Value for Number {
    method &self.value(): i32 {
        return self.value
    }
}

func main(): i32 {
    let number = Number { value: 4 }
    return number.doubled()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn conformance_member_overrides_interface_default() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32 {
        return 1
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit {
    method &self.value(): i32 {
        return 2
    }
}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_competing_interface_defaults() {
    let diagnostics = check_text(
        r#"interface Left {
    pub method &self.value(): i32 {
        return 1
    }
}

interface Right {
    pub method &self.value(): i32 {
        return 2
    }
}

copy struct Unit {
    marker: i32
}

impl Left for Unit {}
impl Right for Unit {}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0451"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_invalid_interface_default_body_against_declaring_contract() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32 {
        return self.missing()
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
            .any(|diagnostic| diagnostic.message.contains("missing")),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_inherent_and_interface_implementation_name_conflict() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32
}

copy struct Unit {
    marker: i32
}

impl Unit {
    pub method &self.value(): i32 {
        return 1
    }
}

impl Value for Unit {
    method &self.value(): i32 {
        return 2
    }
}

func main(): i32 {
    let unit = Unit { marker: 0 }
    return unit.value()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0451"),
        "{diagnostics:?}"
    );
}

#[test]
fn diagnoses_incompatible_conformance_override_of_interface_default() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32 {
        return 1
    }
}

copy struct Unit {
    marker: i32
}

impl Value for Unit {
    method &self.value(): usize {
        return 2
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
fn accepts_explicit_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl Printable for User {
    method &self.print(): i32 {
        return 1
    }
}

func main(): i32 {
    let user = User { id: 1 }
    return user.print()
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_generic_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Source<T> {
    pub method self.get(): T
}

struct Box<T> {
    value: T
}

impl<T> Source<T> for Box<T> {
    method self.get(): T {
        return self.value
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
fn accepts_self_type_in_interface_method_signature() {
    let diagnostics = check_text(
        r#"interface Cloneable {
    pub method &self.clone(): Self
}

struct User {
    id: i32
}

impl Cloneable for User {
    method &self.clone(): User {
        return User { id: self.id }
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
fn diagnoses_interface_parameter_value_type() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

func main(): i32 {
    return 0
}

func render(value: Printable): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `value`"));
    assert!(diagnostics[0].message.contains("Printable"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("concrete"));
}

#[test]
fn diagnoses_interface_borrow_parameter_type() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

func main(): i32 {
    return 0
}

func render(value: &Printable): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `value`"));
    assert!(diagnostics[0].message.contains("&Printable"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("dispatch"));
}

#[test]
fn diagnoses_interface_alias_parameter_value_type() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

type PrintableContract = Printable

func main(): i32 {
    return 0
}

func render(value: PrintableContract): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `value`"));
    assert!(diagnostics[0].message.contains("PrintableContract"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("concrete"));
}

#[test]
fn diagnoses_interface_generic_argument_value_type() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct Box<T> {
    value: T
}

func main(): i32 {
    return 0
}

func render(value: Box<Printable>): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0380");
    assert!(diagnostics[0].message.contains("parameter `value`"));
    assert!(diagnostics[0].message.contains("Box<Printable>"));
    assert!(diagnostics[0].help.as_ref().unwrap().contains("dispatch"));
}

#[test]
fn diagnoses_missing_interface_method() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl Printable for User {}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0425");
    assert!(diagnostics[0].message.contains("print"));
}

#[test]
fn diagnoses_extra_interface_implementation_method() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl Printable for User {
    method &self.print(): i32 {
        return 1
    }

    method &self.debug(): i32 {
        return 2
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0425");
    assert!(diagnostics[0].message.contains("not declared"));
}

#[test]
fn diagnoses_interface_method_signature_mismatch() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl Printable for User {
    method &self.print(): bool {
        return true
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0426");
    assert!(diagnostics[0].message.contains("does not match"));
}

#[test]
fn diagnoses_generic_interface_method_signature_mismatch() {
    let diagnostics = check_text(
        r#"interface Source<T> {
    pub method self.get(): T
}

struct Box<T> {
    value: T
}

impl<T> Source<T> for Box<T> {
    method self.get(): i32 {
        return 0
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0426");
    assert!(diagnostics[0].message.contains("does not match"));
}

#[test]
fn diagnoses_duplicate_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl Printable for User {
    method &self.print(): i32 {
        return 1
    }
}
impl Printable for User {
    method &self.print(): i32 {
        return 2
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0424");
}

#[test]
fn diagnoses_duplicate_generic_interface_conformance_with_renamed_parameters() {
    let diagnostics = check_text(
        r#"interface Source<T> {
    pub method self.get(): T
}

struct Box<T> {
    value: T
}

impl<T> Source<T> for Box<T> {
    method self.get(): T {
        return self.value
    }
}
impl<U> Source<U> for Box<U> {
    method self.get(): U {
        return self.value
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0424");
}

#[test]
fn diagnoses_non_interface_conformance_contract() {
    let diagnostics = check_text(
        r#"struct User {
    id: i32
}

impl User for User {}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0422");
}

#[test]
fn diagnoses_non_nominal_interface_conformance_target() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

type Id = i32

impl Printable for Id {}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0423");
}
