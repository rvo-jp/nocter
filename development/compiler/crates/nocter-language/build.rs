use std::env;
use std::fmt::Write;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=diagnostic-codes.txt");

    let catalog =
        fs::read_to_string("diagnostic-codes.txt").expect("diagnostic-codes.txt must be readable");
    let codes = catalog.lines().collect::<Vec<_>>();
    assert!(
        !codes.is_empty(),
        "diagnostic code catalog must not be empty"
    );

    for code in &codes {
        assert!(
            code.len() == 5
                && code.starts_with('E')
                && code.as_bytes()[1..].iter().all(u8::is_ascii_digit),
            "invalid diagnostic code `{code}`"
        );
    }
    assert!(
        codes.windows(2).all(|pair| pair[0] < pair[1]),
        "diagnostic codes must be unique and lexically sorted"
    );

    let mut variants = String::new();
    let mut strings = String::new();
    let mut all = String::new();
    for code in codes {
        writeln!(variants, "    {code},").expect("writing a String cannot fail");
        writeln!(strings, "            Self::{code} => \"{code}\",")
            .expect("writing a String cannot fail");
        writeln!(all, "        Self::{code},").expect("writing a String cannot fail");
    }
    let generated = format!(
        "\
/// Stable registered Nocter diagnostic code.\n\
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]\n\
pub enum DiagnosticCode {{\n\
{variants}\
}}\n\
\n\
impl DiagnosticCode {{\n\
    /// Every registered public diagnostic code in lexical order.\n\
    pub const ALL: &'static [Self] = &[\n\
{all}\
    ];\n\
\n\
    #[must_use]\n\
    #[allow(clippy::too_many_lines)]\n\
    pub const fn as_str(self) -> &'static str {{\n\
        match self {{\n\
{strings}\
        }}\n\
    }}\n\
}}\n\
\n\
impl std::fmt::Display for DiagnosticCode {{\n\
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n\
        formatter.write_str(self.as_str())\n\
    }}\n\
}}\n"
    );

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must define OUT_DIR"))
        .join("diagnostic_code.rs");
    fs::write(output, generated).expect("generated diagnostic code module must be writable");
}
