use super::*;

#[test]
fn distributed_std_iterator_adapter_surface_passes_check() {
    let project = TempProject::new("distributed-home-iterator-adapters-check");
    let source = project.write_source(
        "iterator_adapters_shape.nct",
        r#"use std/iter.{ChainIter, chain}
use std/iter.{EnumerateIter, Indexed, enumerate}
use std/iter.{count, last}
use std/iter.{SkipIter, TakeIter, skip, take}
use std/iter.{EmptyIter, OnceIter, empty, once}
use std/vec.Vec

func main(): i32 {
    var empty_values: EmptyIter<i32> = empty()
    var one_value: OnceIter<i32> = once(1)
    let chained: ChainIter<EmptyIter<i32>, OnceIter<i32>> = chain(move empty_values, move one_value)
    let limited: TakeIter<ChainIter<EmptyIter<i32>, OnceIter<i32>>> = take(move chained, 1)
    let skipped: SkipIter<TakeIter<ChainIter<EmptyIter<i32>, OnceIter<i32>>>> = skip(move limited, 0)
    let indexed: EnumerateIter<SkipIter<TakeIter<ChainIter<EmptyIter<i32>, OnceIter<i32>>>>> = enumerate(move skipped)
    let values: Vec<Indexed<i32>> = Vec.from_exact_iter(move indexed)
    let item_count: usize = count(once(1))
    let final_item: i32 = last(once(2)) otherwise { return 1 }
    if values.len() == 1 && item_count == 1 && final_item == 2 {
        return 0
    }
    return 1
}
"#,
    );

    assert_success(&nocter_check(&project, &source));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_iterator_adapters_and_vec_builders_run() {
    let project = TempProject::new("distributed-home-iterator-adapters-run");
    let source = project.write_source(
        "iterator_adapters_run.nct",
        r#"use std/iter.chain
use std/iter.enumerate
use std/iter.{count, last}
use std/iter.{skip, take}
use std/iter.once
use std/iter.{ExactSizeIterator, Iterator}
use std/mem.page_allocator
use std/vec.Vec

struct PlainIter {
    next_value: i32
    end: i32
}

impl Iterator for PlainIter {
    type Item = i32

    method &+self.next(): i32? {
        if self.next_value == self.end {
            return none
        }
        let value: i32 = self.next_value
        self.next_value = value + 1
        return value
    }
}

struct ReportedIter {
    next_value: i32
    end: i32
    reported: usize
}

impl Iterator for ReportedIter {
    type Item = i32

    method &+self.next(): i32? {
        if self.next_value == self.end {
            return none
        }
        let value: i32 = self.next_value
        self.next_value = value + 1
        return value
    }

}

impl ExactSizeIterator for ReportedIter {
    method &self.remaining_len(): usize {
        return self.reported
    }
}

func main(): i32 {
    var prefix = take(chain(once(3), once(4)), 2)
    if prefix.remaining_len() != 2 {
        return 1
    }
    let first: i32 = prefix.next() otherwise { return 2 }
    if prefix.remaining_len() != 1 {
        return 3
    }
    let second: i32 = prefix.next() otherwise { return 4 }
    if prefix.remaining_len() != 0 {
        return 5
    }

    var suffix = skip(chain(once(5), once(6)), 1)
    if suffix.remaining_len() != 1 {
        return 6
    }
    let suffix_item: i32 = suffix.next() otherwise { return 7 }

    var indexed = enumerate(chain(once(7), once(8)))
    let indexed_first = indexed.next() otherwise { return 8 }
    let indexed_second = indexed.next() otherwise { return 9 }

    let counted: usize = count(chain(once(9), once(10)))
    let final_item: i32 = last(chain(once(11), once(12))) otherwise { return 10 }

    let grown: Vec<i32> = Vec.from_iter(PlainIter { next_value: 13, end: 15 })
    let reserved: Vec<i32> = Vec.from_exact_iter(take(chain(once(15), once(16)), 2))
    let underreported: Vec<i32> = Vec.from_exact_iter(ReportedIter {
        next_value: 17,
        end: 19,
        reported: 0,
    })
    let overreported: Vec<i32> = Vec.from_exact_iter(ReportedIter {
        next_value: 19,
        end: 21,
        reported: 4,
    })

    if first != 3 { return 11 }
    if second != 4 { return 12 }
    if suffix_item != 6 { return 13 }
    if indexed_first.index != 0 || indexed_first.item != 7 { return 14 }
    if indexed_second.index != 1 || indexed_second.item != 8 { return 15 }
    if counted != 2 || final_item != 12 { return 16 }
    if grown.len() != 2 || (&grown as &[i32])[0] != 13 || (&grown as &[i32])[1] != 14 { return 17 }
    if reserved.len() != 2 || reserved.capacity() != 2 { return 18 }
    if (&reserved as &[i32])[0] != 15 || (&reserved as &[i32])[1] != 16 { return 19 }
    if underreported.len() != 2 || (&underreported as &[i32])[0] != 17 || (&underreported as &[i32])[1] != 18 { return 20 }
    if overreported.len() != 2 || overreported.capacity() != 4 { return 21 }
    if (&overreported as &[i32])[0] != 19 || (&overreported as &[i32])[1] != 20 { return 22 }
    let arena = page_allocator()
    region temporary using arena {
        let regional: Vec<i32> = Vec.from_iter(PlainIter { next_value: 21, end: 23 })
        if regional.len() != 2 || (&regional as &[i32])[1] != 22 { return 23 }
    }
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
fn distributed_std_callable_iterator_defaults_run() {
    let project = TempProject::new("distributed-home-callable-iterator-defaults-run");
    let source = project.write_source(
        "callable_iterator_defaults_run.nct",
        r#"use std/iter.{FoldStep, Iterator}
use std/iter.once
use std/vec.Vec

func main(): i32 {
    var total = 1
    var mapped = once(4).map((&+total; value) {
        total = total + value
        total
    })
    if mapped.remaining_len() != 1 { return 1 }
    let values = mapped.filter((value) { value >= 5 }).take(8).to_vec()
    if total != 5 || values.len() != 1 || (&values as &[i32])[0] != 5 { return 2 }

    let found = once(7).find((value) { value == 7 }) otherwise { return 3 }
    if found != 7 { return 4 }
    if !once(8).any((value) { value == 8 }) { return 5 }
    if !once(9).all((value) { value >= 9 }) { return 6 }

    let folded = once(6).fold(4, (step) { step.accumulator + step.item })
    if folded != 10 { return 7 }

    let marker = String "callback"
    let source = Vec [String "first", String "second"]
    var prefix = source.into_iter().map((move marker; item) { move item }).take(1).to_vec()
    if prefix.len() != 1 { return 8 }
    let first = prefix.pop() otherwise { return 9 }
    if first.len() != 5 { return 10 }
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

#[test]
fn distributed_std_filter_does_not_claim_exact_size() {
    let project = TempProject::new("distributed-home-filter-not-exact-size");
    let source = project.write_source(
        "filter_not_exact_size.nct",
        r#"use std/iter.Iterator
use std/iter.once

func main(): i32 {
    let filtered = once(1).filter((value) { value == 1 })
    return filtered.remaining_len() as i32
}
"#,
    );

    let output = nocter_check(&project, &source);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("remaining_len"), "{stderr}");
    assert!(stderr.contains("no method"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_callback_allocation_uses_current_region() {
    let project = TempProject::new("distributed-home-callback-current-region");
    let source = project.write_source(
        "callback_current_region.nct",
        r#"use std/iter.Iterator
use std/iter.once
use std/mem.page_allocator

func main(): i32 {
    let arena = page_allocator()
    region temporary using arena {
        let text = once(1).map((value) { String "regional" }).last() otherwise { return 1 }
        if text.len() != 8 { return 2 }
    }
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

#[test]
fn distributed_std_callback_result_cannot_hide_region_storage() {
    let project = TempProject::new("distributed-home-callback-region-escape");
    let source = project.write_source(
        "callback_region_escape.nct",
        r#"use std/iter.Iterator
use std/iter.once
use std/mem.page_allocator

func leak(): String {
    let arena = page_allocator()
    region temporary using arena {
        let text = once(1).map((value) { String "regional" }).last() otherwise { return String "fallback" }
        return move text
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("error[E0436]"), "{stderr}");
    assert!(stderr.contains("region `temporary`"), "{stderr}");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn distributed_std_callback_borrow_results_preserve_argument_and_capture_origins() {
    let project = TempProject::new("distributed-home-callback-borrow-provenance");
    let source = project.write_source(
        "callback_borrow_provenance.nct",
        r#"use std/iter.Iterator
use std/iter.once

func from_argument(value: &i32): &i32 from value {
    let result = once(value).map((item) { item }).last() otherwise { return value }
    return result
}

func from_capture(value: &i32): &i32 from value {
    let result = once(0).map((move value; item) { value }).last() otherwise { return value }
    return result
}

func main(): i32 {
    let value = 21
    let argument_result = from_argument(&value)
    let capture_result = from_capture(&value)
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

#[test]
fn distributed_std_iterator_builder_cannot_escape_lexical_region() {
    let project = TempProject::new("distributed-home-iterator-builder-region-escape");
    let source = project.write_source(
        "iterator_builder_region_escape.nct",
        r#"use std/iter.once
use std/mem.page_allocator
use std/vec.Vec

func leak(): Vec<i32> {
    let arena = page_allocator()
    region temporary using arena {
        return Vec.from_iter(once(1))
    }
}

func main(): i32 {
    return 0
}
"#,
    );

    let output = nocter_check(&project, &source);
    assert_eq!(output.status.code(), Some(1), "{}", text(&output.stderr));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0436]"), "{stderr}");
    assert!(stderr.contains("region `temporary`"), "{stderr}");
}
