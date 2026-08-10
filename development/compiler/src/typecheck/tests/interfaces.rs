use super::check_text;

#[test]
fn accepts_required_associated_type_projection_in_generic_code() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
    pub method &+self.next(): Self.Item?
}

struct Buffer<T> {
    marker: i32,
}

conform<T> Source for Buffer<T> {
    type Item = T

    method &+self.next(): T? {
        return none
    }
}

func pull<S>(source: &+S): S.Item? where S: Source {
    return source.next()
}

func main(): void {
    var source = Buffer<i32> { marker: 0 }
    let value: i32? = pull(&+source)
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn normalizes_associated_types_in_concrete_nested_and_where_constrained_positions() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
}

copy struct Pair<T> {
    first: T
    second: T
}

copy struct Numbers {
    value: i32
}

copy struct Box<T> {
    value: T
}

conform Source for Numbers {
    type Item = i32
}

conform<T> Source for Box<T> {
    type Item = T
}

type NumberAlias = Numbers

func concrete(value: Numbers.Item): i32 {
    return value
}

func concrete_generic(value: Box<i32>.Item): i32 {
    return value
}

func aliased(value: NumberAlias.Item): i32 {
    return value
}

func nested<S>(value: Pair<S.Item>): Pair<S.Item> where S: Source {
    return move value
}

func main(): i32 {
    let pair = Pair<i32> { first: 20, second: 22 }
    return concrete(pair.first) + concrete_generic(aliased(pair.second))
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_incomplete_and_non_contract_associated_type_bindings() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
}

struct Buffer {
    marker: i32,
}

conform Source for Buffer {
    type Other = i32
    type Other = bool
}

func main(): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0433")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0434")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0435")
    );
}

#[test]
fn diagnoses_duplicate_and_invalid_associated_type_projections() {
    let diagnostics = check_text(
        r#"interface Left {
    pub type Item
    pub type Item
}

interface Right {
    pub type Item
}

func ambiguous<T>(value: T): T.Item where T: Left + Right {
    return value
}

func unknown<T>(value: T): T.Output where T: Left {
    return value
}

func main(): void {
    return
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0432")
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0436" && diagnostic.message.contains("ambiguous")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0436" && diagnostic.message.contains("no associated type")
    }));
}

#[test]
fn associated_type_bounds_enable_projected_method_calls() {
    let diagnostics = check_text(
        r#"interface Iterator {
    pub type Item
    pub method &+self.next(): Self.Item?
}

interface Iterable {
    pub type Iter: Iterator
    pub method &self.iter(): Self.Iter
}

func first<S>(source: &S): S.Iter.Item? where S: Iterable {
    var iterator = source.iter()
    return iterator.next()
}

func main(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_an_associated_binding_that_violates_its_bound() {
    let diagnostics = check_text(
        r#"interface Iterator {
    pub type Item
}

interface Iterable {
    pub type Iter: Iterator
    pub method &self.iter(): Self.Iter
}

copy struct Bad {
    value: i32
}

conform Iterable for Bad {
    type Iter = i32

    method &self.iter(): i32 {
        return self.value
    }
}

func main(): void {
    return
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0468"),
        "{diagnostics:?}"
    );
}

#[test]
fn where_equality_makes_projected_types_interchangeable() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
}

func align<L, R>(value: R.Item): L.Item where R.Item = L.Item, L: Source, R: Source {
    return value
}

func main(): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn rejects_where_equality_without_an_associated_projection() {
    let diagnostics = check_text(
        r#"func invalid<T, U>(value: T): U where T = U {
    return value
}

func main(): void {
    return
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0466"),
        "{diagnostics:?}"
    );
}

#[test]
fn rejects_call_whose_associated_types_violate_where_equality() {
    let diagnostics = check_text(
        r#"interface Source {
    pub type Item
}

copy struct Integers { marker: i32 }
copy struct Flags { marker: i32 }

conform Source for Integers {
    type Item = i32
}

conform Source for Flags {
    type Item = bool
}

func pair<L, R>(left: L, right: R): void where R.Item = L.Item, L: Source, R: Source {
    return
}

func main(): void {
    pair(Integers { marker: 0 }, Flags { marker: 0 })
    return
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0469"),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_interface_default_without_instance_method() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32 {
        return 7
    }
}

copy struct Unit {
    marker: i32
}

conform Value for Unit {}

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

conform Value for Number {
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

conform Value for Unit {
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

conform Left for Unit {}
conform Right for Unit {}

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
fn diagnoses_inherent_and_conformanceementation_name_conflict() {
    let diagnostics = check_text(
        r#"interface Value {
    pub method &self.value(): i32
}

copy struct Unit {
    marker: i32
}

instance Unit {
    pub method &self.value(): i32 {
        return 1
    }
}

conform Value for Unit {
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

conform Value for Unit {
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

conform Printable for User {
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
    pub method self.get(): T from self
}

struct Box<T> {
    value: T
}

conform<T> Source<T> for Box<T> {
    method self.get(): T from self {
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
fn storage_independent_result_narrows_interface_provenance_contract() {
    let diagnostics = check_text(
        r#"interface Source<T> {
    pub method self.get(): T from self
}

struct Constant {
    value: i32
}

conform Source<i32> for Constant {
    method self.get(): i32 {
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
fn fresh_result_narrows_interface_external_provenance_contract() {
    let diagnostics = check_text(
        r#"primitive fresh(): &i32

interface Source {
    pub method &self.get(): &i32 from self
}

struct Factory {}

conform Source for Factory {
    method &self.get(): &i32 {
        return fresh()
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_explicit_conformance_of_elided_interface_origin() {
    let diagnostics = check_text(
        r#"interface Source {
    pub method &self.get(): &i32
}

struct Holder { value: &i32 }

conform Source for Holder {
    method &self.get(): &i32 from self {
        return self.value
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_elided_interface_origin_in_conformance_body() {
    let diagnostics = check_text(
        r#"interface Source {
    pub method &self.get(): &i32
}

struct Holder { value: &i32 }

conform Source for Holder {
    method &self.get(): &i32 {
        return self.value
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_elided_conformance_that_selects_an_ambiguous_origin() {
    let diagnostics = check_text(
        r#"interface Source {
    pub method &self.get(other: &Self): &Self from self
}

struct Holder { value: &i32 }

conform Source for Holder {
    method &self.get(other: &Self): &Self {
        return other
    }
}

func main(): i32 { return 0 }
"#,
    );

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0444"),
        "{diagnostics:?}"
    );
}

#[test]
fn static_result_narrows_an_interface_without_external_origins() {
    let diagnostics = check_text(
        r#"primitive static_value(): &i32 from static

interface Source {
    pub method &self.get(): &i32
}

struct Factory {}

conform Source for Factory {
    method &self.get(): &i32 from static {
        return static_value()
    }
}

func main(): i32 { return 0 }
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

conform Cloneable for User {
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

conform Printable for User {}

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
fn diagnoses_extra_conformanceementation_method() {
    let diagnostics = check_text(
        r#"interface Printable {
    pub method &self.print(): i32
}

struct User {
    id: i32
}

conform Printable for User {
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

conform Printable for User {
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
    pub method self.get(): T from self
}

struct Box<T> {
    value: T
}

conform<T> Source<T> for Box<T> {
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

conform Printable for User {
    method &self.print(): i32 {
        return 1
    }
}
conform Printable for User {
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
    pub method self.get(): T from self
}

struct Box<T> {
    value: T
}

conform<T> Source<T> for Box<T> {
    method self.get(): T from self {
        return self.value
    }
}
conform<U> Source<U> for Box<U> {
    method self.get(): U from self {
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

conform User for User {}

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

conform Printable for Id {}

func main(): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0423");
}
