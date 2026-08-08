pub(super) fn package(name: &str, library: bool) -> String {
    let executable = (!library).then(|| format!("#executable: {{\n    name: \"{name}\",\n}}\n"));
    format!(
        "//! {name}\n\n#name: \"{name}\"\n#version: \"0.1.0\"\n{}#test: {{ name: \"unit\", module: \"./tests/unit\" }}\n",
        executable.as_deref().unwrap_or("")
    )
}

pub(super) fn executable_source(name: &str) -> String {
    format!(
        "use std/io.print\n\nfunc main(): i32! {{\n    print(\"Hello from {name}\\n\")?\n    return 0\n}}\n"
    )
}

pub(super) fn library_source(name: &str) -> String {
    format!("pub func name(): &str {{\n    return \"{name}\"\n}}\n")
}

pub(super) fn test_source() -> &'static str {
    "//! Package tests.\n\ntest starts {\n    return\n}\n"
}
