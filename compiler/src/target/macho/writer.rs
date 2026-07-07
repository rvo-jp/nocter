use super::codesign::{adhoc_signature_size, write_adhoc_signature};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableImage {
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn write_arm64_macos_executable(text: &[u8]) -> ExecutableImage {
    let layout = Layout::new(text.len());
    let mut writer = Writer::new();

    write_header(&mut writer, &layout);
    write_pagezero_segment(&mut writer);
    write_text_segment(&mut writer, &layout, text.len());
    write_linkedit_segment(&mut writer, &layout);
    write_symtab(&mut writer);
    write_dysymtab(&mut writer);
    write_load_dylinker(&mut writer);
    write_build_version(&mut writer);
    write_entry_point(&mut writer, &layout);
    write_code_signature_command(&mut writer, &layout);
    writer.pad_to(layout.text_file_offset);
    writer.write_bytes(text);
    writer.pad_to(layout.code_signature_offset);
    let signature = write_adhoc_signature(&writer.bytes, CODE_SIGNATURE_IDENTIFIER, text.len());
    assert_eq!(signature.len(), layout.code_signature_size);
    writer.write_bytes(&signature);
    assert_eq!(writer.len(), layout.file_size);

    ExecutableImage {
        bytes: writer.finish(),
    }
}

fn write_header(writer: &mut Writer, layout: &Layout) {
    writer.write_u32(MH_MAGIC_64);
    writer.write_u32(CPU_TYPE_ARM64);
    writer.write_u32(CPU_SUBTYPE_ARM64_ALL);
    writer.write_u32(MH_EXECUTE);
    writer.write_u32(LOAD_COMMAND_COUNT);
    writer.write_u32(layout.load_commands_size);
    writer.write_u32(MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL | MH_PIE);
    writer.write_u32(0);
}

fn write_pagezero_segment(writer: &mut Writer) {
    writer.write_u32(LC_SEGMENT_64);
    writer.write_u32(SEGMENT_COMMAND_64_SIZE);
    writer.write_fixed_name("__PAGEZERO");
    writer.write_u64(0);
    writer.write_u64(PAGEZERO_SIZE);
    writer.write_u64(0);
    writer.write_u64(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
}

fn write_text_segment(writer: &mut Writer, layout: &Layout, text_len: usize) {
    writer.write_u32(LC_SEGMENT_64);
    writer.write_u32(SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE);
    writer.write_fixed_name("__TEXT");
    writer.write_u64(TEXT_BASE_ADDRESS);
    writer.write_u64(layout.text_segment_file_size as u64);
    writer.write_u64(0);
    writer.write_u64(layout.text_segment_file_size as u64);
    writer.write_u32(VM_PROT_READ | VM_PROT_EXECUTE);
    writer.write_u32(VM_PROT_READ | VM_PROT_EXECUTE);
    writer.write_u32(1);
    writer.write_u32(0);

    writer.write_fixed_name("__text");
    writer.write_fixed_name("__TEXT");
    writer.write_u64(TEXT_BASE_ADDRESS + layout.text_file_offset as u64);
    writer.write_u64(text_len as u64);
    writer.write_u32(layout.text_file_offset as u32);
    writer.write_u32(2);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(S_REGULAR | S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
}

fn write_linkedit_segment(writer: &mut Writer, layout: &Layout) {
    writer.write_u32(LC_SEGMENT_64);
    writer.write_u32(SEGMENT_COMMAND_64_SIZE);
    writer.write_fixed_name("__LINKEDIT");
    writer.write_u64(TEXT_BASE_ADDRESS + layout.text_segment_file_size as u64);
    writer.write_u64(layout.linkedit_segment_size as u64);
    writer.write_u64(layout.code_signature_offset as u64);
    writer.write_u64(layout.code_signature_size as u64);
    writer.write_u32(VM_PROT_READ);
    writer.write_u32(VM_PROT_READ);
    writer.write_u32(0);
    writer.write_u32(0);
}

fn write_symtab(writer: &mut Writer) {
    writer.write_u32(LC_SYMTAB);
    writer.write_u32(SYMTAB_COMMAND_SIZE);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
}

fn write_dysymtab(writer: &mut Writer) {
    writer.write_u32(LC_DYSYMTAB);
    writer.write_u32(DYSYMTAB_COMMAND_SIZE);
    for _ in 0..DYSYMTAB_FIELD_COUNT {
        writer.write_u32(0);
    }
}

fn write_load_dylinker(writer: &mut Writer) {
    writer.write_u32(LC_LOAD_DYLINKER);
    writer.write_u32(LOAD_DYLINKER_COMMAND_SIZE);
    writer.write_u32(DYLINKER_PATH_OFFSET);
    writer.write_bytes(DYLINKER_PATH);
    writer.pad_to(writer.len().next_multiple_of(8));
}

fn write_build_version(writer: &mut Writer) {
    writer.write_u32(LC_BUILD_VERSION);
    writer.write_u32(BUILD_VERSION_COMMAND_SIZE);
    writer.write_u32(PLATFORM_MACOS);
    writer.write_u32(MACOS_11_0_0);
    writer.write_u32(MACOS_11_0_0);
    writer.write_u32(0);
}

fn write_entry_point(writer: &mut Writer, layout: &Layout) {
    writer.write_u32(LC_MAIN);
    writer.write_u32(ENTRY_POINT_COMMAND_SIZE);
    writer.write_u64(layout.text_file_offset as u64);
    writer.write_u64(0);
}

fn write_code_signature_command(writer: &mut Writer, layout: &Layout) {
    writer.write_u32(LC_CODE_SIGNATURE);
    writer.write_u32(CODE_SIGNATURE_COMMAND_SIZE);
    writer.write_u32(layout.code_signature_offset as u32);
    writer.write_u32(layout.code_signature_size as u32);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    load_commands_size: u32,
    text_file_offset: usize,
    text_segment_file_size: usize,
    code_signature_offset: usize,
    code_signature_size: usize,
    linkedit_segment_size: usize,
    file_size: usize,
}

impl Layout {
    fn new(text_len: usize) -> Self {
        let load_commands_size = SEGMENT_COMMAND_64_SIZE
            + SEGMENT_COMMAND_64_SIZE
            + SECTION_64_SIZE
            + SEGMENT_COMMAND_64_SIZE
            + SYMTAB_COMMAND_SIZE
            + DYSYMTAB_COMMAND_SIZE
            + LOAD_DYLINKER_COMMAND_SIZE
            + BUILD_VERSION_COMMAND_SIZE
            + ENTRY_POINT_COMMAND_SIZE
            + CODE_SIGNATURE_COMMAND_SIZE;
        let text_file_offset = align_usize(MACH_HEADER_64_SIZE + load_commands_size as usize, 16);
        let text_segment_file_size = align_usize(text_file_offset + text_len, PAGE_SIZE);
        let code_signature_offset = text_segment_file_size;
        let code_signature_size =
            adhoc_signature_size(code_signature_offset, CODE_SIGNATURE_IDENTIFIER);
        let linkedit_segment_size = align_usize(code_signature_size, PAGE_SIZE);
        let file_size = code_signature_offset + code_signature_size;

        Self {
            load_commands_size,
            text_file_offset,
            text_segment_file_size,
            code_signature_offset,
            code_signature_size,
            linkedit_segment_size,
            file_size,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self::default()
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn write_fixed_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        assert!(bytes.len() <= FIXED_NAME_SIZE);
        self.bytes.extend_from_slice(bytes);
        self.bytes
            .resize(self.bytes.len() + FIXED_NAME_SIZE - bytes.len(), 0);
    }

    fn pad_to(&mut self, size: usize) {
        assert!(self.bytes.len() <= size);
        self.bytes.resize(size, 0);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn align_usize(value: usize, alignment: usize) -> usize {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

const PAGE_SIZE: usize = 0x4000;
const PAGEZERO_SIZE: u64 = 0x1_0000_0000;
const TEXT_BASE_ADDRESS: u64 = 0x1_0000_0000;
const FIXED_NAME_SIZE: usize = 16;
const CODE_SIGNATURE_IDENTIFIER: &str = "nocter";

const MACH_HEADER_64_SIZE: usize = 32;
const SEGMENT_COMMAND_64_SIZE: u32 = 72;
const SECTION_64_SIZE: u32 = 80;
const SYMTAB_COMMAND_SIZE: u32 = 24;
const DYSYMTAB_COMMAND_SIZE: u32 = 80;
const LOAD_DYLINKER_COMMAND_SIZE: u32 = 32;
const BUILD_VERSION_COMMAND_SIZE: u32 = 24;
const ENTRY_POINT_COMMAND_SIZE: u32 = 24;
const CODE_SIGNATURE_COMMAND_SIZE: u32 = 16;
const LOAD_COMMAND_COUNT: u32 = 9;
const DYSYMTAB_FIELD_COUNT: usize = 18;

const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
const MH_EXECUTE: u32 = 0x2;
const MH_NOUNDEFS: u32 = 0x1;
const MH_DYLDLINK: u32 = 0x4;
const MH_TWOLEVEL: u32 = 0x80;
const MH_PIE: u32 = 0x20_0000;

const LC_REQ_DYLD: u32 = 0x8000_0000;
const LC_SYMTAB: u32 = 0x2;
const LC_DYSYMTAB: u32 = 0xb;
const LC_SEGMENT_64: u32 = 0x19;
const LC_CODE_SIGNATURE: u32 = 0x1d;
const LC_LOAD_DYLINKER: u32 = 0xe;
const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;
const LC_BUILD_VERSION: u32 = 0x32;

const DYLINKER_PATH_OFFSET: u32 = 12;
const DYLINKER_PATH: &[u8] = b"/usr/lib/dyld\0";

const VM_PROT_READ: u32 = 0x1;
const VM_PROT_EXECUTE: u32 = 0x4;

const S_REGULAR: u32 = 0x0;
const S_ATTR_PURE_INSTRUCTIONS: u32 = 0x8000_0000;
const S_ATTR_SOME_INSTRUCTIONS: u32 = 0x0000_0400;

const PLATFORM_MACOS: u32 = 1;
const MACOS_11_0_0: u32 = 0x000b_0000;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_mach_header_64_for_arm64_execute() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);

        assert_eq!(read_u32(&image.bytes, 0), MH_MAGIC_64);
        assert_eq!(read_u32(&image.bytes, 4), CPU_TYPE_ARM64);
        assert_eq!(read_u32(&image.bytes, 8), CPU_SUBTYPE_ARM64_ALL);
        assert_eq!(read_u32(&image.bytes, 12), MH_EXECUTE);
        assert_eq!(read_u32(&image.bytes, 16), LOAD_COMMAND_COUNT);
        assert_eq!(read_u32(&image.bytes, 20), 496);
        assert_eq!(
            read_u32(&image.bytes, 24),
            MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL | MH_PIE
        );
        assert_eq!(read_u32(&image.bytes, 28), 0);
    }

    #[test]
    fn writes_pagezero_segment() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let segment = MACH_HEADER_64_SIZE;

        assert_eq!(read_u32(&image.bytes, segment), LC_SEGMENT_64);
        assert_eq!(read_u32(&image.bytes, segment + 4), SEGMENT_COMMAND_64_SIZE);
        assert_eq!(
            fixed_name(&image.bytes[segment + 8..segment + 24]),
            "__PAGEZERO"
        );
        assert_eq!(read_u64(&image.bytes, segment + 24), 0);
        assert_eq!(read_u64(&image.bytes, segment + 32), PAGEZERO_SIZE);
        assert_eq!(read_u64(&image.bytes, segment + 40), 0);
        assert_eq!(read_u64(&image.bytes, segment + 48), 0);
        assert_eq!(read_u32(&image.bytes, segment + 56), 0);
        assert_eq!(read_u32(&image.bytes, segment + 60), 0);
        assert_eq!(read_u32(&image.bytes, segment + 64), 0);
        assert_eq!(read_u32(&image.bytes, segment + 68), 0);
    }

    #[test]
    fn writes_text_segment_and_section() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let segment = MACH_HEADER_64_SIZE + SEGMENT_COMMAND_64_SIZE as usize;
        let section = segment + SEGMENT_COMMAND_64_SIZE as usize;
        let text_offset = align_usize(MACH_HEADER_64_SIZE + 496, 16);

        assert_eq!(read_u32(&image.bytes, segment), LC_SEGMENT_64);
        assert_eq!(
            read_u32(&image.bytes, segment + 4),
            SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE
        );
        assert_eq!(
            fixed_name(&image.bytes[segment + 8..segment + 24]),
            "__TEXT"
        );
        assert_eq!(read_u64(&image.bytes, segment + 24), TEXT_BASE_ADDRESS);
        assert_eq!(read_u64(&image.bytes, segment + 32), PAGE_SIZE as u64);
        assert_eq!(read_u64(&image.bytes, segment + 40), 0);
        assert_eq!(read_u64(&image.bytes, segment + 48), PAGE_SIZE as u64);
        assert_eq!(read_u32(&image.bytes, segment + 64), 1);

        assert_eq!(fixed_name(&image.bytes[section..section + 16]), "__text");
        assert_eq!(
            fixed_name(&image.bytes[section + 16..section + 32]),
            "__TEXT"
        );
        assert_eq!(
            read_u64(&image.bytes, section + 32),
            TEXT_BASE_ADDRESS + text_offset as u64
        );
        assert_eq!(read_u64(&image.bytes, section + 40), 4);
        assert_eq!(read_u32(&image.bytes, section + 48), text_offset as u32);
        assert_eq!(read_u32(&image.bytes, section + 52), 2);
        assert_eq!(
            read_u32(&image.bytes, section + 64),
            S_REGULAR | S_ATTR_PURE_INSTRUCTIONS | S_ATTR_SOME_INSTRUCTIONS
        );
    }

    #[test]
    fn writes_linkedit_segment() {
        let text = [0xc0, 0x03, 0x5f, 0xd6];
        let image = write_arm64_macos_executable(&text);
        let layout = Layout::new(text.len());
        let segment = MACH_HEADER_64_SIZE
            + SEGMENT_COMMAND_64_SIZE as usize
            + (SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE) as usize;

        assert_eq!(read_u32(&image.bytes, segment), LC_SEGMENT_64);
        assert_eq!(read_u32(&image.bytes, segment + 4), SEGMENT_COMMAND_64_SIZE);
        assert_eq!(
            fixed_name(&image.bytes[segment + 8..segment + 24]),
            "__LINKEDIT"
        );
        assert_eq!(
            read_u64(&image.bytes, segment + 24),
            TEXT_BASE_ADDRESS + layout.text_segment_file_size as u64
        );
        assert_eq!(
            read_u64(&image.bytes, segment + 32),
            layout.linkedit_segment_size as u64
        );
        assert_eq!(
            read_u64(&image.bytes, segment + 40),
            layout.code_signature_offset as u64
        );
        assert_eq!(
            read_u64(&image.bytes, segment + 48),
            layout.code_signature_size as u64
        );
        assert_eq!(read_u32(&image.bytes, segment + 56), VM_PROT_READ);
        assert_eq!(read_u32(&image.bytes, segment + 60), VM_PROT_READ);
        assert_eq!(read_u32(&image.bytes, segment + 64), 0);
    }

    #[test]
    fn writes_empty_symbol_tables() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let symtab = MACH_HEADER_64_SIZE
            + SEGMENT_COMMAND_64_SIZE as usize
            + (SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE) as usize
            + SEGMENT_COMMAND_64_SIZE as usize;
        let dysymtab = symtab + SYMTAB_COMMAND_SIZE as usize;

        assert_eq!(read_u32(&image.bytes, symtab), LC_SYMTAB);
        assert_eq!(read_u32(&image.bytes, symtab + 4), SYMTAB_COMMAND_SIZE);
        assert_eq!(read_u32(&image.bytes, symtab + 8), 0);
        assert_eq!(read_u32(&image.bytes, symtab + 12), 0);
        assert_eq!(read_u32(&image.bytes, symtab + 16), 0);
        assert_eq!(read_u32(&image.bytes, symtab + 20), 0);

        assert_eq!(read_u32(&image.bytes, dysymtab), LC_DYSYMTAB);
        assert_eq!(read_u32(&image.bytes, dysymtab + 4), DYSYMTAB_COMMAND_SIZE);
        assert!(
            image.bytes[dysymtab + 8..dysymtab + DYSYMTAB_COMMAND_SIZE as usize]
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn writes_load_dylinker() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let dylinker = MACH_HEADER_64_SIZE
            + SEGMENT_COMMAND_64_SIZE as usize
            + (SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE) as usize
            + SEGMENT_COMMAND_64_SIZE as usize
            + SYMTAB_COMMAND_SIZE as usize
            + DYSYMTAB_COMMAND_SIZE as usize;

        assert_eq!(read_u32(&image.bytes, dylinker), LC_LOAD_DYLINKER);
        assert_eq!(
            read_u32(&image.bytes, dylinker + 4),
            LOAD_DYLINKER_COMMAND_SIZE
        );
        assert_eq!(read_u32(&image.bytes, dylinker + 8), DYLINKER_PATH_OFFSET);
        assert_eq!(
            &image.bytes[dylinker + DYLINKER_PATH_OFFSET as usize
                ..dylinker + DYLINKER_PATH_OFFSET as usize + DYLINKER_PATH.len()],
            DYLINKER_PATH
        );
    }

    #[test]
    fn writes_build_version_and_entry_point() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let build_version = MACH_HEADER_64_SIZE
            + SEGMENT_COMMAND_64_SIZE as usize
            + (SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE) as usize
            + SEGMENT_COMMAND_64_SIZE as usize
            + SYMTAB_COMMAND_SIZE as usize
            + DYSYMTAB_COMMAND_SIZE as usize
            + LOAD_DYLINKER_COMMAND_SIZE as usize;
        let entry_point = build_version + BUILD_VERSION_COMMAND_SIZE as usize;
        let text_offset = align_usize(MACH_HEADER_64_SIZE + 496, 16);

        assert_eq!(read_u32(&image.bytes, build_version), LC_BUILD_VERSION);
        assert_eq!(
            read_u32(&image.bytes, build_version + 4),
            BUILD_VERSION_COMMAND_SIZE
        );
        assert_eq!(read_u32(&image.bytes, build_version + 8), PLATFORM_MACOS);
        assert_eq!(read_u32(&image.bytes, build_version + 12), MACOS_11_0_0);
        assert_eq!(read_u32(&image.bytes, build_version + 16), MACOS_11_0_0);
        assert_eq!(read_u32(&image.bytes, build_version + 20), 0);

        assert_eq!(read_u32(&image.bytes, entry_point), LC_MAIN);
        assert_eq!(
            read_u32(&image.bytes, entry_point + 4),
            ENTRY_POINT_COMMAND_SIZE
        );
        assert_eq!(read_u64(&image.bytes, entry_point + 8), text_offset as u64);
        assert_eq!(read_u64(&image.bytes, entry_point + 16), 0);
    }

    #[test]
    fn writes_code_signature_command() {
        let text = [0xc0, 0x03, 0x5f, 0xd6];
        let image = write_arm64_macos_executable(&text);
        let layout = Layout::new(text.len());
        let code_signature = MACH_HEADER_64_SIZE
            + SEGMENT_COMMAND_64_SIZE as usize
            + (SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE) as usize
            + SEGMENT_COMMAND_64_SIZE as usize
            + SYMTAB_COMMAND_SIZE as usize
            + DYSYMTAB_COMMAND_SIZE as usize
            + LOAD_DYLINKER_COMMAND_SIZE as usize
            + BUILD_VERSION_COMMAND_SIZE as usize
            + ENTRY_POINT_COMMAND_SIZE as usize;

        assert_eq!(read_u32(&image.bytes, code_signature), LC_CODE_SIGNATURE);
        assert_eq!(
            read_u32(&image.bytes, code_signature + 4),
            CODE_SIGNATURE_COMMAND_SIZE
        );
        assert_eq!(
            read_u32(&image.bytes, code_signature + 8),
            layout.code_signature_offset as u32
        );
        assert_eq!(
            read_u32(&image.bytes, code_signature + 12),
            layout.code_signature_size as u32
        );
    }

    #[test]
    fn places_text_and_appends_code_signature() {
        let text = [0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6];
        let image = write_arm64_macos_executable(&text);
        let layout = Layout::new(text.len());

        assert_eq!(
            &image.bytes[layout.text_file_offset..layout.text_file_offset + text.len()],
            text
        );
        assert!(
            image.bytes[layout.text_file_offset + text.len()..layout.code_signature_offset]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(image.bytes.len(), layout.file_size);
        assert_eq!(
            read_be_u32(&image.bytes, layout.code_signature_offset),
            0xfade_0cc0
        );
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        let mut value = [0; 4];
        value.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_le_bytes(value)
    }

    fn read_u64(bytes: &[u8], offset: usize) -> u64 {
        let mut value = [0; 8];
        value.copy_from_slice(&bytes[offset..offset + 8]);
        u64::from_le_bytes(value)
    }

    fn read_be_u32(bytes: &[u8], offset: usize) -> u32 {
        let mut value = [0; 4];
        value.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_be_bytes(value)
    }

    fn fixed_name(bytes: &[u8]) -> &str {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).unwrap()
    }
}
