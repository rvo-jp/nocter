use super::check_text;

#[test]
fn source_defined_index_operator_types_readonly_access() {
    let diagnostics = check_text(
        r#"struct Buffer {
    values: [i32; 2],
}

instance Buffer {
    pub operator (&self[index: usize]): &i32 {
        return &self.values[index]
    }
}

func read(buffer: &Buffer): i32 {
    return buffer[0]
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn source_defined_index_operator_types_readwrite_access() {
    let diagnostics = check_text(
        r#"struct Buffer {
    values: [i32; 2],
}

instance Buffer {
    pub operator (&self[index: usize]): &i32 {
        return &self.values[index]
    }

    pub operator (&+self[index: usize]): &+i32 {
        return &+self.values[index]
    }
}

func replace(buffer: &+Buffer): void {
    buffer[0] = 42
    return
}
"#,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn rejects_index_operator_result_with_the_wrong_capability() {
    let diagnostics = check_text(
        r#"struct Buffer {}

instance Buffer {
    pub operator (&self[index: usize]): i32 {
        return 0
    }
}
"#,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "E0470"
            && diagnostic
                .message
                .contains("readonly index operator return type must be `&T`")
    }));
}
