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

impl Holder {
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

impl File {
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

impl File {
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

impl Counter {
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

impl File {
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

impl Holder {
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
fn diagnoses_maybe_uninitialized_after_catch_fallthrough_moves() {
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

    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0385"
                && diagnostic.message.contains("may be uninitialized")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "E0337" && diagnostic.message.contains("catch")),
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
