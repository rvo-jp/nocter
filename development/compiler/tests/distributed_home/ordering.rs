use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_source_and_standard_strict_ordering_run() {
    let project = TempProject::new("distributed-home-strict-ordering");
    let source = project.write_source(
        "index.nct",
        r#"use std/string.String
use std/vec.Vec

struct Rank { value: i32 }

instance Rank {
    pub operator (&self < other: &Self): bool {
        return self.value < other.value
    }
}

func earlier<T>(left: &T, right: &T): bool where (&T < &T): bool {
    return left < right
}

func main(): i32 {
    let low = Rank { value: 2 }
    let high = Rank { value: 7 }
    if !earlier(&low, &high) { return 1 }
    if !(high > low) || high <= low || low >= high { return 2 }

    let alpha = String "alpha"
    let beta = String "beta"
    let alpha_ref = &alpha
    let beta_ref = &beta
    let alpha_text: &str = "alpha"
    let beta_text: &str = "beta"
    if !(alpha_ref < beta_ref) { return 3 }
    if !(alpha_text < beta_ref) { return 4 }
    if !(alpha_ref < beta_text) { return 5 }
    if !(alpha_text < beta_text) { return 6 }

    let first = Vec [1, 2]
    let second = Vec [1, 3]
    let first_ref = &first
    let second_ref = &second
    if !(first_ref < second_ref) { return 7 }
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
