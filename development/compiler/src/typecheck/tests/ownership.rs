use super::check_text;

#[test]
fn diagnoses_use_after_move_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    let length = take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_double_move_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    let first = take(move text)
    let second = take(move text)
    return first + second
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_double_move_of_move_only_fixed_array() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    let values: [Text; 1] = [Text { len: 42 }]
    let first = consume(move values)
    let second = consume(move values)
    return first + second
}

func consume(values: [Text; 1]): i32 {
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn invalid_outer_move_operand_does_not_consume_nested_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    let invalid = move take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0370");
    assert!(diagnostics[0].message.contains("binding"));
}

#[test]
fn tracks_named_field_moves_independently() {
    let diagnostics = check_text(
        r#"struct Resource { value: i32 }

destruct Resource(&+self) { return }

struct Pair {
    first: Resource
    second: Resource
}

func consume(value: Resource): i32 {
    return value.value
}

func main(): i32 {
    let pair = Pair {
        first: Resource { value: 1 },
        second: Resource { value: 2 },
    }
    let second = consume(move pair.second)
    let first = consume(move pair.first)
    return first + second
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_reuse_of_a_moved_named_field() {
    let diagnostics = check_text(
        r#"struct Resource { value: i32 }

destruct Resource(&+self) { return }

struct Pair {
    first: Resource
    second: Resource
}

func consume(value: Resource): i32 {
    return value.value
}

func main(): i32 {
    let pair = Pair {
        first: Resource { value: 1 },
        second: Resource { value: 2 },
    }
    let first = consume(move pair.first)
    return first + consume(move pair.first)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("pair.first"));
}

#[test]
fn diagnoses_use_after_explicit_drop_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    drop text
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("dropped"));
}

#[test]
fn diagnoses_double_explicit_drop_of_non_copy_struct() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    drop text
    drop text
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("drop"));
    assert!(diagnostics[0].message.contains("dropped"));
}

#[test]
fn diagnoses_double_explicit_drop_of_move_only_fixed_array() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    let values: [Text; 1] = [Text { len: 42 }]
    drop values
    drop values
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("drop"));
    assert!(diagnostics[0].message.contains("dropped"));
}

#[test]
fn diagnoses_explicit_drop_of_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let pair = Pair { left: 1, right: 2 }
    drop pair
    return pair.left + pair.right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0386");
    assert!(diagnostics[0].message.contains("Pair"));
}

#[test]
fn diagnoses_move_of_copy_struct() {
    let diagnostics = check_text(
        r#"copy struct Pair {
    left: i32
    right: i32
}

func main(): i32 {
    let pair = Pair { left: 20, right: 22 }
    let copied = move pair
    return pair.left + copied.right
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0394");
    assert!(diagnostics[0].message.contains("Pair"));
}

#[test]
fn accepts_explicit_drop_of_non_copy_generic_copy_struct_instantiation() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

copy struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<Text> { value: Text { len: 42 } }
    drop box
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_mutation_while_implicitly_borrowed_receiver_result_is_live() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

struct Holder {
    value: &Text
}

instance Text {
    method &self.hold(): Holder {
        return Holder { value: self }
    }
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let holder = text.hold()
    text = Text { value: 2 }
    inspect(holder.value)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("holder"));
}

#[test]
fn returned_aggregate_preserves_readwrite_borrow_conflicts() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

struct Holder {
    value: &+Text
}

func hold(value: &+Text): Holder {
    return Holder { value: value }
}

func inspect(value: &Text): void {
    return
}

func touch(value: &+Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let holder = hold(&+text)
    inspect(&text)
    touch(holder.value)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("read"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("holder"));
}

#[test]
fn accepts_move_of_non_copy_generic_copy_struct_instantiation() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

copy struct Box<T> {
    value: T
}

func main(): i32 {
    let box = Box<Text> { value: Text { len: 42 } }
    let moved = move box
    return moved.value.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_move_of_generic_parameter() {
    let diagnostics = check_text(
        r#"func identity<T>(value: T): T {
    return move value
}

func main(): i32 {
    return identity(42)
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_use_after_move_of_generic_parameter() {
    let diagnostics = check_text(
        r#"func duplicate<T>(value: T): T {
    let moved = move value
    return value
}

func main(): i32 {
    return duplicate(42)
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("value"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn accepts_var_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text { start: 1, len: 20, capacity: 3 }
    let first = take(move text)
    text = Text { start: 4, len: 22, capacity: 6 }
    return first + text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_move_after_readonly_borrow_last_use() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    let text = Text { len: 42 }
    let read = &text
    inspect(read)
    return take(move text)
}

func inspect(text: &Text): void {
    return
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_move_while_readonly_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    let text = Text { len: 42 }
    let read = &text
    let length = take(move text)
    inspect(read)
    return length
}

func inspect(text: &Text): void {
    return
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn explicit_coercion_result_keeps_the_source_loan_until_last_use() {
    let diagnostics = check_text(
        r#"struct Text {
    value: &str
}

instance Text {
    pub coerce &self as &str from self { return self.value }
}

func inspect(value: &str): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let text = Text { value: "hello" }
    let view = &text as &str
    take(move text)
    inspect(view)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("view"));
}

#[test]
fn explicit_coercion_loan_ends_after_its_last_use() {
    let diagnostics = check_text(
        r#"struct Text {
    value: &str
}

instance Text {
    pub coerce &self as &str from self { return self.value }
}

func inspect(value: &str): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let text = Text { value: "hello" }
    let view = &text as &str
    inspect(view)
    take(move text)
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn receiver_coercion_method_result_keeps_the_original_source_loan() {
    let diagnostics = check_text(
        r#"struct View { value: &str }
struct Text { view: View }

instance Text {
    pub coerce &self as &View { return &self.view }
}

instance View {
    pub method &self.project(): &str { return self.value }
}

func inspect(value: &str): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let text = Text { view: View { value: "hello" } }
    let projected = text.project()
    take(move text)
    inspect(projected)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("projected"));
}

#[test]
fn receiver_coercion_method_loan_ends_after_its_last_use() {
    let diagnostics = check_text(
        r#"struct View { value: &str }
struct Text { view: View }

instance Text { pub coerce &self as &View { return &self.view } }
instance View { pub method &self.project(): &str { return self.value } }

func inspect(value: &str): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let text = Text { view: View { value: "hello" } }
    let projected = text.project()
    inspect(projected)
    take(move text)
    return 0
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn explicit_coercion_of_a_branch_result_keeps_every_possible_source_loan() {
    let diagnostics = check_text(
        r#"struct Text {
    value: &str
}

instance Text {
    pub coerce &self as &str from self { return self.value }
}

func inspect(value: &str): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let first = Text { value: "first" }
    let second = Text { value: "second" }
    let view = (if true { &first } else { &second }) as &str
    take(move second)
    inspect(view)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("second"));
    assert!(diagnostics[0].message.contains("view"));
}

#[test]
fn borrow_valued_match_keeps_every_possible_source_loan() {
    let diagnostics = check_text(
        r#"enum Choice { first second }

struct Text {
    value: &str
}

func inspect(value: &Text): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let choice = Choice.first
    let first = Text { value: "first" }
    let second = Text { value: "second" }
    let view = match choice {
        Choice.first { &first }
        Choice.second { &second }
    }
    take(move second)
    inspect(view)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("second"));
    assert!(diagnostics[0].message.contains("view"));
}

#[test]
fn borrow_valued_if_is_keeps_every_possible_source_loan() {
    let diagnostics = check_text(
        r#"enum Choice { first second }

struct Text {
    value: &str
}

func inspect(value: &Text): void { return }
func take(value: Text): void { return }

func main(): i32 {
    let choice = Choice.first
    let first = Text { value: "first" }
    let second = Text { value: "second" }
    let view = if choice is Choice.first { &first } else { &second }
    take(move second)
    inspect(view)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("second"));
    assert!(diagnostics[0].message.contains("view"));
}

#[test]
fn accepts_move_before_unreachable_borrow_use() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    let text = Text { len: 42 }
    let read = &text
    return take(move text)
    inspect(read)
    return 0
}

func inspect(text: &Text): void {
    return
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_drop_while_readwrite_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    var text = Text { len: 42 }
    let write = &+text
    drop text
    touch(write)
    return 0
}

func touch(text: &+Text): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("drop"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("write"));
}

#[test]
fn accepts_readwrite_borrow_after_readonly_borrow_last_use() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    var text = Text { len: 42 }
    let read = &text
    inspect(read)
    let write = &+text
    touch(write)
    return 0
}

func inspect(text: &Text): void {
    return
}

func touch(text: &+Text): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_mutation_while_helper_returned_borrow_is_used_later() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func same(value: &Text): &Text {
    return value
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let read = same(&text)
    text = Text { value: 2 }
    inspect(read)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("assign"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn accepts_mutation_after_helper_returned_borrow_last_use() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func same(value: &Text): &Text {
    return value
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var text = Text { value: 1 }
    let read = same(&text)
    inspect(read)
    text = Text { value: 2 }
    return text.value
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn helper_result_constrains_every_possible_input_origin() {
    let diagnostics = check_text(
        r#"struct Text {
    value: i32
}

func choose(first: &Text, second: &Text, use_first: bool): &Text {
    if use_first {
        return first
    }
    return second
}

func inspect(value: &Text): void {
    return
}

func main(): i32 {
    var left = Text { value: 1 }
    let right = Text { value: 2 }
    let read = choose(&left, &right, true)
    left = Text { value: 3 }
    inspect(read)
    return 0
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("left"));
}

#[test]
fn diagnoses_readwrite_borrow_while_readonly_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    var text = Text { len: 42 }
    let read = &text
    let write = &+text
    inspect(read)
    touch(write)
    return 0
}

func inspect(text: &Text): void {
    return
}

func touch(text: &+Text): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("readwrite borrow"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn diagnoses_assignment_while_readonly_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Text {
    len: i32
}

func main(): i32 {
    var text = Text { len: 42 }
    let read = &text
    text = Text { len: 7 }
    inspect(read)
    return 0
}

func inspect(text: &Text): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("assign"));
    assert!(diagnostics[0].message.contains("text"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn diagnoses_owned_method_receiver_move_while_readonly_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Holder {
    value: i32
}

instance Holder {
    method self.take(): i32 {
        return self.value
    }
}

func main(): i32 {
    let holder = Holder { value: 21 }
    let read = &holder
    let value = holder.take()
    inspect(read)
    return value
}

func inspect(holder: &Holder): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("move"));
    assert!(diagnostics[0].message.contains("holder"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn diagnoses_readwrite_method_receiver_while_readonly_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

instance File {
    method &+self.write(): void {
        return
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    let read = &file
    file.write()
    inspect(read)
    return 0
}

func inspect(file: &File): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("readwrite borrow"));
    assert!(diagnostics[0].message.contains("file"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn diagnoses_readwrite_field_method_receiver_while_readonly_field_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

struct Holder {
    file: File
}

instance File {
    method &+self.write(): void {
        return
    }
}

func main(): i32 {
    var holder = Holder { file: File { fd: 1 } }
    let read = &holder.file
    holder.file.write()
    inspect(read)
    return 0
}

func inspect(file: &File): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("readwrite borrow"));
    assert!(diagnostics[0].message.contains("holder"));
    assert!(diagnostics[0].message.contains("read"));
}

#[test]
fn accepts_assignment_to_disjoint_field_while_field_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Name {
    len: i32
}

struct User {
    name: Name
    count: i32
}

func main(): i32 {
    var user = User { name: Name { len: 5 }, count: 0 }
    let name = &user.name
    user.count = 1
    inspect(name)
    return user.count
}

func inspect(name: &Name): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_assignment_to_borrowed_field_used_later() {
    let diagnostics = check_text(
        r#"struct Name {
    len: i32
}

struct User {
    name: Name
    count: i32
}

func main(): i32 {
    var user = User { name: Name { len: 5 }, count: 0 }
    let name = &user.name
    user.name = Name { len: 7 }
    inspect(name)
    return user.count
}

func inspect(name: &Name): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("assign"));
    assert!(diagnostics[0].message.contains("user.name"));
    assert!(diagnostics[0].message.contains("name"));
}

#[test]
fn diagnoses_whole_assignment_while_field_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Name {
    len: i32
}

struct User {
    name: Name
    count: i32
}

func main(): i32 {
    var user = User { name: Name { len: 5 }, count: 0 }
    let name = &user.name
    user = User { name: Name { len: 7 }, count: 1 }
    inspect(name)
    return user.count
}

func inspect(name: &Name): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("assign"));
    assert!(diagnostics[0].message.contains("user"));
    assert!(diagnostics[0].message.contains("name"));
}

#[test]
fn accepts_read_of_disjoint_field_while_readwrite_field_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Name {
    len: i32
}

struct User {
    name: Name
    count: i32
}

func main(): i32 {
    var user = User { name: Name { len: 5 }, count: 0 }
    let name = &+user.name
    let count = user.count
    touch(name)
    return count
}

func touch(name: &+Name): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_readwrite_method_receiver_on_disjoint_field_while_field_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct Name {
    len: i32
}

struct Counter {
    value: i32
}

struct User {
    name: Name
    counter: Counter
}

instance Counter {
    method &+self.increment(): void {
        self.value = self.value + 1
        return
    }
}

func main(): i32 {
    var user = User { name: Name { len: 5 }, counter: Counter { value: 0 } }
    let name = &user.name
    user.counter.increment()
    inspect(name)
    return user.counter.value
}

func inspect(name: &Name): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_readonly_method_receiver_while_readwrite_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

instance File {
    method &self.fd_value(): i32 {
        return self.fd
    }
}

func main(): i32 {
    var file = File { fd: 1 }
    let write = &+file
    let fd = file.fd_value()
    touch(write)
    return fd
}

func touch(file: &+File): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("readonly borrow"));
    assert!(diagnostics[0].message.contains("file"));
    assert!(diagnostics[0].message.contains("write"));
}

#[test]
fn diagnoses_field_read_while_readwrite_borrow_used_later() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

func main(): i32 {
    var file = File { fd: 1 }
    let write = &+file
    let fd = file.fd
    touch(write)
    return fd
}

func touch(file: &+File): void {
    return
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0434");
    assert!(diagnostics[0].message.contains("use"));
    assert!(diagnostics[0].message.contains("file"));
    assert!(diagnostics[0].message.contains("write"));
}

#[test]
fn accepts_field_read_after_readwrite_borrow_last_use() {
    let diagnostics = check_text(
        r#"struct File {
    fd: i32
}

func main(): i32 {
    var file = File { fd: 1 }
    let write = &+file
    touch(write)
    return file.fd
}

func touch(file: &+File): void {
    return
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_use_after_owned_method_receiver_move() {
    let diagnostics = check_text(
        r#"struct Holder {
    value: i32
}

instance Holder {
    method self.take(): i32 {
        return self.value
    }
}

func main(): i32 {
    let holder = Holder { value: 21 }
    let value = holder.take()
    return value + holder.value
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("holder"));
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_one_if_branch_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    if true {
        let length = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_uninitialized_after_both_if_branches_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    if true {
        let first = take(move text)
    } else {
        let second = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("moved"));
}

#[test]
fn diagnoses_uninitialized_after_if_branches_move_and_drop() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    if true {
        let length = take(move text)
    } else {
        drop text
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn accepts_if_branch_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text { start: 1, len: 20, capacity: 3 }
    if true {
        let first = take(move text)
        text = Text { start: 4, len: 22, capacity: 6 }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_uninitialized_after_match_branches_move_and_drop() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    drop_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text { start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }

        _ {
            drop text
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn diagnoses_uninitialized_after_exhaustive_match_without_wildcard_fallback_branches_move_and_drop()
{
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    drop_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text { start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }

        Choice.drop_it {
            drop text
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("is uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_match_without_wildcard_fallback_moves() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    keep_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text { start: 1, len: 42, capacity: 3 }
    match choice {
        Choice.move_it {
            let length = take(move text)
        }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_match_expression_arm_moves() {
    let diagnostics = check_text(
        r#"enum Choice {
    move_it
    keep_it
}

struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let choice = Choice.move_it
    let text = Text { start: 1, len: 42, capacity: 3 }
    let value = match choice {
        Choice.move_it { take(move text) }
        _ { 0 }
    }
    return text.len + value
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn diagnoses_maybe_uninitialized_after_value_producing_catch_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    let value = fallible() catch error {
        let moved = take(move text)
        moved
    }
    return text.len + value
}

func fallible(): i32! {
    return 1
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0385"
                && diagnostic.message.contains("may be uninitialized")),
        "{diagnostics:?}"
    );
}

#[test]
fn accepts_unreachable_use_after_returning_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    return take(move text)
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_borrow_use_after_terminal_if_as_unreachable() {
    let diagnostics = check_text(
        r#"struct Box {
    value: i32
}

func main(): i32 {
    var box = Box { value: 1 }
    let view = &box
    if true {
        box.value = 2
        return 0
    } else {
        return 1
    }
    view.value
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_borrow_use_after_exhaustive_switch_as_unreachable() {
    let diagnostics = check_text(
        r#"struct Box {
    value: i32
}

enum Choice {
    yes
    no
}

func main(): i32 {
    var box = Box { value: 1 }
    let view = &box
    let choice = Choice.yes
    match choice {
        Choice.yes {
            box.value = 2
            return 0
        }
        Choice.no {
            return 1
        }
    }
    view.value
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn accepts_if_branch_return_after_move_without_poisoning_fallthrough() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    if true {
        return take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_maybe_uninitialized_after_while_body_moves() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    while true {
        let length = take(move text)
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("may be uninitialized"));
}

#[test]
fn accepts_while_body_reinitialization_after_move() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    var text = Text { start: 1, len: 20, capacity: 3 }
    while true {
        let length = take(move text)
        text = Text { start: 4, len: 22, capacity: 6 }
    }
    return text.len
}

func take(text: Text): i32 {
    return text.len
}
"#,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn diagnoses_uninitialized_after_loop_break_drops() {
    let diagnostics = check_text(
        r#"struct Text {
    start: i32
    len: i32
    capacity: i32
}

func main(): i32 {
    let text = Text { start: 1, len: 42, capacity: 3 }
    loop {
        drop text
        break
    }
    return text.len
}
"#,
    );

    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "E0385");
    assert!(diagnostics[0].message.contains("dropped"));
}
