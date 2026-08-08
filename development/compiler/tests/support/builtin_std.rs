use std::fs;
use std::path::Path;

/// Writes the compiler-required built-in method implementation units into a
/// deliberately minimal test home.
pub(crate) fn write_builtin_type_surfaces(home: &Path) {
    let std = home.join("std");
    fs::create_dir_all(&std).unwrap();
    fs::write(
        std.join("str.nct"),
        r#"pub(nocter) primitive str_len_raw(value: &str): usize

impl str {
    pub method &self.len(): usize {
        return str_len_raw(self)
    }

    pub method &self.is_empty(): bool {
        return self.len() == 0
    }
}
"#,
    )
    .unwrap();
    fs::write(
        std.join("slice.nct"),
        r#"pub(nocter) primitive slice_len_raw<T>(value: &[T]): usize

impl<T> [T] {
    pub method &self.len(): usize {
        return slice_len_raw(self)
    }

    pub method &self.is_empty(): bool {
        return self.len() == 0
    }
}
"#,
    )
    .unwrap();
}
