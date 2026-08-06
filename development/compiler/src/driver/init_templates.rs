pub(super) fn executable(name: &str) -> String {
    format!(
        "//! {name}\n\n#name: \"{name}\"\n#version: \"0.1.0\"\n#executable: {{\n    name: \"{name}\",\n}}\n\nuse std/io.print\n\nfunc main(): i32! {{\n    print(\"Hello from {name}\\n\")?\n    return 0\n}}\n\ntest starts {{\n    return\n}}\n"
    )
}

pub(super) fn library(name: &str) -> String {
    format!(
        "//! {name}\n\n#name: \"{name}\"\n#version: \"0.1.0\"\n\npub func name(): &str {{\n    return \"{name}\"\n}}\n\ntest exposes_name {{\n    return\n}}\n"
    )
}
