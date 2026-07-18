use super::check_text;

#[test]
fn accepts_explicit_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method (value: &Self).print(): i32
}

struct User {
    id: i32
}

impl User {
    pub method (value: &Self).print(): i32 {
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
fn diagnoses_missing_interface_method() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method (value: &Self).print(): i32
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
    pub method (value: &Self).print(): i32
}

struct User {
    id: i32
}

impl User {
    method (value: &Self).print(): i32 {
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
    pub method (value: &Self).print(): i32
}

struct User {
    id: i32
}

impl User {
    pub method (value: &Self).print(): bool {
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
fn diagnoses_duplicate_interface_conformance() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method (value: &Self).print(): i32
}

struct User {
    id: i32
}

impl User {
    pub method (value: &Self).print(): i32 {
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
    pub method (value: &Self).print(): i32
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
