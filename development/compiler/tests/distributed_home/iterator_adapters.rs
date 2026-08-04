use super::*;

#[test]
fn distributed_std_iterator_adapter_surface_passes_check() {
    let project = TempProject::new("distributed-home-iterator-adapters-check");
    let source = project.write_source(
        "iterator_adapters_shape.nct",
        r#"use std/iter/chain.{ChainIter, chain}
use std/iter/enumerate.{EnumerateIter, Indexed, enumerate}
use std/iter/ops.{count, last}
use std/iter/range.{SkipIter, TakeIter, skip, take}
use std/iter/sources.{EmptyIter, OnceIter, empty, once}
use std/vec.Vec

func main(): i32 {
    var empty_values: EmptyIter<i32> = empty()
    var one_value: OnceIter<i32> = once(1)
    let chained: ChainIter<i32, EmptyIter<i32>, OnceIter<i32>> = chain(move empty_values, move one_value)
    let limited: TakeIter<i32, ChainIter<i32, EmptyIter<i32>, OnceIter<i32>>> = take(move chained, 1)
    let skipped: SkipIter<i32, TakeIter<i32, ChainIter<i32, EmptyIter<i32>, OnceIter<i32>>>> = skip(move limited, 0)
    let indexed: EnumerateIter<i32, SkipIter<i32, TakeIter<i32, ChainIter<i32, EmptyIter<i32>, OnceIter<i32>>>>> = enumerate(move skipped)
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
        r#"use std/iter/chain.chain
use std/iter/enumerate.enumerate
use std/iter/ops.{count, last}
use std/iter/range.{skip, take}
use std/iter/sources.once
use std/iter.{ExactSizeIterator, Iterator}
use std/mem.page_allocator
use std/vec.Vec

struct PlainIter {
    next_value: i32
    end: i32
}

impl PlainIter {
    pub method &+self.next(): i32? {
        if self.next_value == self.end {
            return none
        }
        let value: i32 = self.next_value
        self.next_value = value + 1
        return value
    }
}

impl Iterator<i32> for PlainIter

struct ReportedIter {
    next_value: i32
    end: i32
    reported: usize
}

impl ReportedIter {
    pub method &+self.next(): i32? {
        if self.next_value == self.end {
            return none
        }
        let value: i32 = self.next_value
        self.next_value = value + 1
        return value
    }

    pub method &self.remaining_len(): usize {
        return self.reported
    }
}

impl Iterator<i32> for ReportedIter
impl ExactSizeIterator<i32> for ReportedIter

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
    if grown.len() != 2 || grown.view()[0] != 13 || grown.view()[1] != 14 { return 17 }
    if reserved.len() != 2 || reserved.capacity() != 2 { return 18 }
    if reserved.view()[0] != 15 || reserved.view()[1] != 16 { return 19 }
    if underreported.len() != 2 || underreported.view()[0] != 17 || underreported.view()[1] != 18 { return 20 }
    if overreported.len() != 2 || overreported.capacity() != 4 { return 21 }
    if overreported.view()[0] != 19 || overreported.view()[1] != 20 { return 22 }
    let arena = page_allocator()
    region temporary using arena {
        let regional: Vec<i32> = Vec.from_iter(PlainIter { next_value: 21, end: 23 })
        if regional.len() != 2 || regional.view()[1] != 22 { return 23 }
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
use std/iter/sources.once

func main(): i32 {
    var total = 1
    var mapped = once(4).map((&+total; value) {
        total = total + value
        total
    })
    if mapped.remaining_len() != 1 { return 1 }
    let values = mapped.filter((value) { value >= 5 }).take(8).to_vec()
    if total != 5 || values.len() != 1 || values.view()[0] != 5 { return 2 }

    let found = once(7).find((value) { value == 7 }) otherwise { return 3 }
    if found != 7 { return 4 }
    if !once(8).any((value) { value == 8 }) { return 5 }
    if !once(9).all((value) { value >= 9 }) { return 6 }

    let folded = once(6).fold(4, (step) { step.accumulator + step.item })
    if folded != 10 { return 7 }
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
        r#"use std/iter/sources.once
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
