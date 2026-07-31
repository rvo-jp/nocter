use super::check_text;

#[test]
fn accepts_explicit_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl User {
    pub method &self.print(): i32 {
        return 1
    }
}

impl Printable for User

func main(): i32 {
    return 0
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

impl<U> Box<U> {
    pub method self.get(): U {
        return self.value
    }
}

impl<T> Source<T> for Box<T>

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

impl User {
    pub method &self.clone(): User {
        return User { id: self.id }
    }
}

impl Cloneable for User

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

impl Printable for User

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
fn diagnoses_private_interface_method_implementation() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

impl User {
    method &self.print(): i32 {
        return 1
    }
}

impl Printable for User

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0425");
    assert!(diagnostics[0].message.contains("must be public"));
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

impl User {
    pub method &self.print(): bool {
        return true
    }
}

impl Printable for User

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

impl<U> Box<U> {
    pub method self.get(): i32 {
        return 0
    }
}

impl<T> Source<T> for Box<T>

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

impl User {
    pub method &self.print(): i32 {
        return 1
    }
}

impl Printable for User
impl Printable for User

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

impl<U> Box<U> {
    pub method self.get(): U {
        return self.value
    }
}

impl<T> Source<T> for Box<T>
impl<U> Source<U> for Box<U>

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

impl User for User

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

impl Printable for Id

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0423");
}
