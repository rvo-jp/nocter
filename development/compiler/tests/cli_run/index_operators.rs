use super::*;

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn declared_index_operators_run_directly_and_through_a_generic_requirement() {
    let project = TempProject::new("cli-run-declared-index-operators");
    let source = project.write_source(
        "index.nct",
        r#"struct Buffer {
    values: [i32; 1],
}

instance Buffer {
    pub operator (&self[index: usize]): &i32 {
        return &self.values[index]
    }

    pub operator (&+self[index: usize]): &+i32 {
        return &+self.values[index]
    }
}

struct Wrapper {
    buffer: Buffer,
}

coerce Wrapper {
    pub &self as &Buffer from self {
        return &self.buffer
    }
}

copy struct Pair {
    value: i32,
}

struct PairBuffer {
    values: [Pair; 1],
}

instance PairBuffer {
    pub operator (&self[index: usize]): &Pair {
        return &self.values[index]
    }

    pub operator (&+self[index: usize]): &+Pair {
        return &+self.values[index]
    }
}

func at<C, V>(container: &C, index: usize, marker: &V): V where copy V, (&C[usize]): &V {
    return container[index]
}

func read(value: &i32): i32 {
    return 7
}

func main(): i32 {
    var buffer = Buffer { values: [3] }
    buffer[0] = 7
    var pairs = PairBuffer { values: [Pair { value: 2 }] }
    pairs[0] = Pair { value: 8 }
    let pair = pairs[0]
    let wrapper = Wrapper { buffer: Buffer { values: [7] } }
    let marker: i32 = 0
    return buffer[0] + pair.value + read(&buffer[0]) + wrapper[0] + at(&wrapper, 0, &marker)
}
"#,
    );

    let output = nocter(&project, ["run", source.to_str().unwrap()]);

    assert_eq!(
        output.status.code(),
        Some(36),
        "stdout:\n{}\nstderr:\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
}
