#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableImage {
    pub(crate) bytes: Vec<u8>,
}

pub(crate) fn write_arm64_macos_executable(text: &[u8]) -> ExecutableImage {
    let layout = Layout::new(text.len());
    let mut writer = Writer::new();

    write_header(&mut writer, &layout);
    write_text_segment(&mut writer, &layout, text.len());
    write_build_version(&mut writer);
    write_entry_point(&mut writer, &layout);
    writer.pad_to(layout.text_file_offset);
    writer.write_bytes(text);
    writer.pad_to(layout.file_size);

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

fn write_text_segment(writer: &mut Writer, layout: &Layout, text_len: usize) {
    writer.write_u32(LC_SEGMENT_64);
    writer.write_u32(SEGMENT_COMMAND_64_SIZE + SECTION_64_SIZE);
    writer.write_fixed_name("__TEXT");
    writer.write_u64(TEXT_BASE_ADDRESS);
    writer.write_u64(layout.segment_size);
    writer.write_u64(0);
    writer.write_u64(layout.segment_size);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Layout {
    load_commands_size: u32,
    text_file_offset: usize,
    file_size: usize,
    segment_size: u64,
}

impl Layout {
    fn new(text_len: usize) -> Self {
        let load_commands_size = SEGMENT_COMMAND_64_SIZE
            + SECTION_64_SIZE
            + BUILD_VERSION_COMMAND_SIZE
            + ENTRY_POINT_COMMAND_SIZE;
        let text_file_offset = align_usize(MACH_HEADER_64_SIZE + load_commands_size as usize, 16);
        let file_size = align_usize(text_file_offset + text_len, PAGE_SIZE);

        Self {
            load_commands_size,
            text_file_offset,
            file_size,
            segment_size: file_size as u64,
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
const TEXT_BASE_ADDRESS: u64 = 0x1_0000_0000;
const FIXED_NAME_SIZE: usize = 16;

const MACH_HEADER_64_SIZE: usize = 32;
const SEGMENT_COMMAND_64_SIZE: u32 = 72;
const SECTION_64_SIZE: u32 = 80;
const BUILD_VERSION_COMMAND_SIZE: u32 = 24;
const ENTRY_POINT_COMMAND_SIZE: u32 = 24;
const LOAD_COMMAND_COUNT: u32 = 3;

const MH_MAGIC_64: u32 = 0xfeed_facf;
const CPU_TYPE_ARM64: u32 = 0x0100_000c;
const CPU_SUBTYPE_ARM64_ALL: u32 = 0;
const MH_EXECUTE: u32 = 0x2;
const MH_NOUNDEFS: u32 = 0x1;
const MH_DYLDLINK: u32 = 0x4;
const MH_TWOLEVEL: u32 = 0x80;
const MH_PIE: u32 = 0x20_0000;

const LC_REQ_DYLD: u32 = 0x8000_0000;
const LC_SEGMENT_64: u32 = 0x19;
const LC_MAIN: u32 = 0x28 | LC_REQ_DYLD;
const LC_BUILD_VERSION: u32 = 0x32;

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
        assert_eq!(read_u32(&image.bytes, 20), 200);
        assert_eq!(
            read_u32(&image.bytes, 24),
            MH_NOUNDEFS | MH_DYLDLINK | MH_TWOLEVEL | MH_PIE
        );
        assert_eq!(read_u32(&image.bytes, 28), 0);
    }

    #[test]
    fn writes_text_segment_and_section() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let segment = MACH_HEADER_64_SIZE;
        let section = segment + SEGMENT_COMMAND_64_SIZE as usize;
        let text_offset = align_usize(MACH_HEADER_64_SIZE + 200, 16);

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
    fn writes_build_version_and_entry_point() {
        let image = write_arm64_macos_executable(&[0xc0, 0x03, 0x5f, 0xd6]);
        let build_version =
            MACH_HEADER_64_SIZE + SEGMENT_COMMAND_64_SIZE as usize + SECTION_64_SIZE as usize;
        let entry_point = build_version + BUILD_VERSION_COMMAND_SIZE as usize;
        let text_offset = align_usize(MACH_HEADER_64_SIZE + 200, 16);

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
    fn places_text_at_entry_offset_and_pads_to_page() {
        let text = [0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6];
        let image = write_arm64_macos_executable(&text);
        let text_offset = align_usize(MACH_HEADER_64_SIZE + 200, 16);

        assert_eq!(&image.bytes[text_offset..text_offset + text.len()], text);
        assert_eq!(image.bytes.len(), PAGE_SIZE);
        assert!(
            image.bytes[text_offset + text.len()..]
                .iter()
                .all(|byte| *byte == 0)
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

    fn fixed_name(bytes: &[u8]) -> &str {
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end]).unwrap()
    }
}
