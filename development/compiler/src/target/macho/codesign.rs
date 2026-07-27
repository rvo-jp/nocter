pub(super) fn write_adhoc_signature(
    code: &[u8],
    identifier: &str,
    executable_size: usize,
) -> Vec<u8> {
    let code_directory = write_code_directory(code, identifier, executable_size);
    let superblob_size = SUPERBLOB_HEADER_SIZE + BLOB_INDEX_SIZE + code_directory.len();

    let mut writer = BigEndianWriter::new();
    writer.write_u32(CSMAGIC_EMBEDDED_SIGNATURE);
    writer.write_u32(superblob_size as u32);
    writer.write_u32(1);
    writer.write_u32(CSSLOT_CODEDIRECTORY);
    writer.write_u32((SUPERBLOB_HEADER_SIZE + BLOB_INDEX_SIZE) as u32);
    writer.write_bytes(&code_directory);

    let bytes = writer.finish();
    debug_assert_eq!(bytes.len(), superblob_size);
    bytes
}

pub(super) fn adhoc_signature_size(code_limit: usize, identifier: &str) -> usize {
    let code_slots = code_limit.div_ceil(CODE_SIGN_PAGE_SIZE);
    let identifier_len = identifier.len() + 1;
    let hash_offset = align_usize(CODE_DIRECTORY_HEADER_SIZE + identifier_len, 4);
    SUPERBLOB_HEADER_SIZE + BLOB_INDEX_SIZE + hash_offset + code_slots * SHA256_HASH_SIZE
}

fn write_code_directory(code: &[u8], identifier: &str, executable_size: usize) -> Vec<u8> {
    let code_limit = u32::try_from(code.len()).expect("Mach-O code limit must fit in u32");
    let code_slots = code.len().div_ceil(CODE_SIGN_PAGE_SIZE);
    let identifier_len = identifier.len() + 1;
    let ident_offset = CODE_DIRECTORY_HEADER_SIZE;
    let hash_offset = align_usize(ident_offset + identifier_len, 4);
    let code_directory_size = hash_offset + code_slots * SHA256_HASH_SIZE;

    let mut writer = BigEndianWriter::new();
    writer.write_u32(CSMAGIC_CODEDIRECTORY);
    writer.write_u32(code_directory_size as u32);
    writer.write_u32(CS_SUPPORTSEXECSEG);
    writer.write_u32(CS_ADHOC | CS_LINKER_SIGNED);
    writer.write_u32(hash_offset as u32);
    writer.write_u32(ident_offset as u32);
    writer.write_u32(0);
    writer.write_u32(code_slots as u32);
    writer.write_u32(code_limit);
    writer.write_u8(SHA256_HASH_SIZE as u8);
    writer.write_u8(CS_HASHTYPE_SHA256);
    writer.write_u8(0);
    writer.write_u8(CODE_SIGN_PAGE_SIZE_BITS);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u32(0);
    writer.write_u64(0);
    writer.write_u64(0);
    writer.write_u64(executable_size as u64);
    writer.write_u64(CS_EXECSEG_MAIN_BINARY);
    writer.write_bytes(identifier.as_bytes());
    writer.write_u8(0);
    writer.pad_to(hash_offset);

    for page_index in 0..code_slots {
        let start = page_index * CODE_SIGN_PAGE_SIZE;
        let end = usize::min(start + CODE_SIGN_PAGE_SIZE, code.len());
        writer.write_bytes(&sha256(&code[start..end]));
    }

    let bytes = writer.finish();
    debug_assert_eq!(bytes.len(), code_directory_size);
    bytes
}

fn sha256(input: &[u8]) -> [u8; SHA256_HASH_SIZE] {
    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = SHA256_INITIAL_STATE;
    let mut schedule = [0u32; 64];

    for chunk in message.chunks_exact(64) {
        for (index, word) in schedule.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }

        for index in 16..64 {
            let s0 = schedule[index - 15].rotate_right(7)
                ^ schedule[index - 15].rotate_right(18)
                ^ (schedule[index - 15] >> 3);
            let s1 = schedule[index - 2].rotate_right(17)
                ^ schedule[index - 2].rotate_right(19)
                ^ (schedule[index - 2] >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_ROUND_CONSTANTS[index])
                .wrapping_add(schedule[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    let mut output = [0; SHA256_HASH_SIZE];
    for (index, word) in state.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BigEndianWriter {
    bytes: Vec<u8>,
}

impl BigEndianWriter {
    fn new() -> Self {
        Self::default()
    }

    fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
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

const CODE_SIGN_PAGE_SIZE: usize = 4096;
const CODE_SIGN_PAGE_SIZE_BITS: u8 = 12;
const SHA256_HASH_SIZE: usize = 32;

const SUPERBLOB_HEADER_SIZE: usize = 12;
const BLOB_INDEX_SIZE: usize = 8;
const CODE_DIRECTORY_HEADER_SIZE: usize = 88;

const CSMAGIC_CODEDIRECTORY: u32 = 0xfade_0c02;
const CSMAGIC_EMBEDDED_SIGNATURE: u32 = 0xfade_0cc0;
const CS_SUPPORTSEXECSEG: u32 = 0x20400;
const CSSLOT_CODEDIRECTORY: u32 = 0;
const CS_HASHTYPE_SHA256: u8 = 2;
const CS_ADHOC: u32 = 0x0000_0002;
const CS_LINKER_SIGNED: u32 = 0x0002_0000;
const CS_EXECSEG_MAIN_BINARY: u64 = 0x1;

const SHA256_INITIAL_STATE: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const SHA256_ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_empty_input_with_sha256() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hashes_known_input_with_sha256() {
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn writes_superblob_and_code_directory_headers() {
        let code = vec![0; 4096];
        let signature = write_adhoc_signature(&code, "nocter", 8);

        assert_eq!(read_be_u32(&signature, 0), CSMAGIC_EMBEDDED_SIGNATURE);
        assert_eq!(read_be_u32(&signature, 4), signature.len() as u32);
        assert_eq!(read_be_u32(&signature, 8), 1);
        assert_eq!(read_be_u32(&signature, 12), CSSLOT_CODEDIRECTORY);
        assert_eq!(read_be_u32(&signature, 16), 20);
        assert_eq!(read_be_u32(&signature, 20), CSMAGIC_CODEDIRECTORY);
        assert_eq!(read_be_u32(&signature, 28), CS_SUPPORTSEXECSEG);
        assert_eq!(read_be_u32(&signature, 32), CS_ADHOC | CS_LINKER_SIGNED);
        assert_eq!(signature.len(), adhoc_signature_size(code.len(), "nocter"));
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn read_be_u32(bytes: &[u8], offset: usize) -> u32 {
        let mut value = [0; 4];
        value.copy_from_slice(&bytes[offset..offset + 4]);
        u32::from_be_bytes(value)
    }
}
