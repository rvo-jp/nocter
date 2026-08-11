use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_vec_equality_contains_and_position_run() {
    let project = TempProject::new("distributed-home-vec-equality");
    let source = project.write_source(
        "index.nct",
        r#"use std/vec.Vec

struct Token {
    value: i32,
}

instance Token {
    pub operator (&self == other: &Self): bool {
        return self.value == other.value
    }
}

func same(left: &Vec<Token>, right: &Vec<Token>): bool {
    return left == right
}

func main(): i32 {
    let numbers = Vec [1, 2, 3]
    let expected_number = 2
    if !numbers.contains(&expected_number) { return 9 }
    let left = Vec [Token { value: 1 }, Token { value: 2 }]
    let right = Vec [Token { value: 1 }, Token { value: 2 }]
    let different = Vec [Token { value: 1 }, Token { value: 3 }]
    let needle = Token { value: 2 }
    let iterated = Vec [Token { value: 1 }, Token { value: 2 }]
    let positioned = Vec [Token { value: 1 }, Token { value: 2 }]
    if !same(&left, &right) { return 1 }
    if same(&left, &different) { return 2 }
    if !left.contains(&needle) { return 3 }
    let position: usize = left.position(&needle) otherwise { return 4 }
    if position != 1 { return 5 }
    if !(move iterated).into_iter().contains(&needle) { return 6 }
    let iterated_position: usize = (move positioned).into_iter().position(&needle) otherwise { return 7 }
    if iterated_position != 1 { return 8 }
    return 42
}
"#,
    );

    let output = nocter_run(&project, &source);
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
fn distributed_iterator_contains_drops_each_move_only_item_once() {
    let project = TempProject::new("distributed-home-iterator-equality-cleanup");
    let source = project.write_source(
        "index.nct",
        r#"use std/io.print
use std/vec.Vec

struct Token {
    label: &str
}

instance Token {
    pub operator (&self == other: &Self): bool {
        return self.label == other.label
    }
}

destruct Token(&+self) {
    print(self.label)!
    return
}

func main(): i32 {
    let expected = Token { label: "B" }
    let values = Vec [
        Token { label: "A" },
        Token { label: "B" },
        Token { label: "C" },
        Token { label: "D" },
    ]
    if !(move values).into_iter().contains(&expected) {
        return 1
    }
    drop expected
    return 42
}
"#,
    );

    let output = nocter_run(&project, &source);
    assert_eq!(
        output.status.code(),
        Some(42),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(output.stdout, b"ABDCB");
    assert!(output.stderr.is_empty());
}
