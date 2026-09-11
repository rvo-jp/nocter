macro_rules! source_layout {
    ($extension:literal, $module_root_stem:literal) => {
        /// File extension of authored Nocter source, without the leading dot.
        pub const SOURCE_FILE_EXTENSION: &str = $extension;

        /// Portable suffix of authored Nocter source files.
        pub const SOURCE_FILE_SUFFIX: &str = concat!(".", $extension);

        /// File name that establishes one directory module root.
        pub const MODULE_ROOT_FILE_NAME: &str = concat!($module_root_stem, ".", $extension);

        /// Recursive LSP glob selecting authored Nocter source files.
        pub const SOURCE_FILE_GLOB: &str = concat!("**/*.", $extension);
    };
}

source_layout!("nct", "index");

#[cfg(test)]
mod tests {
    use super::{
        MODULE_ROOT_FILE_NAME, SOURCE_FILE_EXTENSION, SOURCE_FILE_GLOB, SOURCE_FILE_SUFFIX,
    };

    #[test]
    fn derived_layout_spellings_remain_consistent() {
        assert_eq!(SOURCE_FILE_SUFFIX, format!(".{SOURCE_FILE_EXTENSION}"));
        assert!(MODULE_ROOT_FILE_NAME.ends_with(SOURCE_FILE_SUFFIX));
        assert!(SOURCE_FILE_GLOB.ends_with(SOURCE_FILE_SUFFIX));
    }
}
