//! Dependency-free deterministic hashing used by compiler-owned formats and identities.

const INITIAL: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND: [u32; 64] = [
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

/// Computes the SHA-256 digest of `input`.
#[must_use]
pub fn sha256(input: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(input);
    digest.finish()
}

/// Incremental SHA-256 computation for compiler-owned streaming formats.
#[derive(Clone, Debug)]
pub struct Sha256 {
    state: [u32; 8],
    buffer: [u8; 64],
    buffered: usize,
    byte_length: u64,
}

impl Sha256 {
    /// Starts one empty digest computation.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: INITIAL,
            buffer: [0; 64],
            buffered: 0,
            byte_length: 0,
        }
    }

    /// Adds the next bytes in their exact order.
    pub fn update(&mut self, mut input: &[u8]) {
        self.byte_length = self.byte_length.wrapping_add(input.len() as u64);
        if self.buffered != 0 {
            let copied = (64 - self.buffered).min(input.len());
            self.buffer[self.buffered..self.buffered + copied].copy_from_slice(&input[..copied]);
            self.buffered += copied;
            input = &input[copied..];
            if self.buffered != 64 {
                return;
            }
            compress(&mut self.state, &self.buffer);
            self.buffered = 0;
        }
        let complete = input.len() / 64 * 64;
        for block in input[..complete].chunks_exact(64) {
            compress(&mut self.state, block);
        }
        let remainder = &input[complete..];
        self.buffer[..remainder.len()].copy_from_slice(remainder);
        self.buffered = remainder.len();
    }

    /// Finishes the digest without retaining mutable computation state.
    #[must_use]
    pub fn finish(mut self) -> [u8; 32] {
        let bit_length = self.byte_length.wrapping_mul(8);
        self.buffer[self.buffered] = 0x80;
        self.buffered += 1;
        if self.buffered > 56 {
            self.buffer[self.buffered..].fill(0);
            compress(&mut self.state, &self.buffer);
            self.buffered = 0;
        }
        self.buffer[self.buffered..56].fill(0);
        self.buffer[56..].copy_from_slice(&bit_length.to_be_bytes());
        compress(&mut self.state, &self.buffer);

        let mut output = [0_u8; 32];
        for (destination, word) in output.chunks_exact_mut(4).zip(self.state) {
            destination.copy_from_slice(&word.to_be_bytes());
        }
        output
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

fn compress(state: &mut [u32; 8], block: &[u8]) {
    let mut schedule = [0_u32; 64];
    for (word, bytes) in schedule.iter_mut().zip(block.chunks_exact(4)) {
        *word = u32::from_be_bytes(bytes.try_into().expect("chunk has four bytes"));
    }
    for index in 16..64 {
        let left = schedule[index - 15];
        let right = schedule[index - 2];
        let small_zero = left.rotate_right(7) ^ left.rotate_right(18) ^ (left >> 3);
        let small_one = right.rotate_right(17) ^ right.rotate_right(19) ^ (right >> 10);
        schedule[index] = schedule[index - 16]
            .wrapping_add(small_zero)
            .wrapping_add(schedule[index - 7])
            .wrapping_add(small_one);
    }

    let mut working = *state;
    for (constant, word) in ROUND.into_iter().zip(schedule) {
        round(&mut working, constant, word);
    }
    for (state, working) in state.iter_mut().zip(working) {
        *state = state.wrapping_add(working);
    }
}

fn round(state: &mut [u32; 8], constant: u32, word: u32) {
    let [
        first_state,
        second_state,
        third_state,
        fourth_state,
        fifth_state,
        sixth_state,
        seventh_state,
        eighth_state,
    ] = *state;
    let big_one =
        fifth_state.rotate_right(6) ^ fifth_state.rotate_right(11) ^ fifth_state.rotate_right(25);
    let choose = (fifth_state & sixth_state) ^ (!fifth_state & seventh_state);
    let first = eighth_state
        .wrapping_add(big_one)
        .wrapping_add(choose)
        .wrapping_add(constant)
        .wrapping_add(word);
    let big_zero =
        first_state.rotate_right(2) ^ first_state.rotate_right(13) ^ first_state.rotate_right(22);
    let majority =
        (first_state & second_state) ^ (first_state & third_state) ^ (second_state & third_state);
    let second = big_zero.wrapping_add(majority);
    *state = [
        first.wrapping_add(second),
        first_state,
        second_state,
        third_state,
        fourth_state.wrapping_add(first),
        fifth_state,
        sixth_state,
        seventh_state,
    ];
}

#[cfg(test)]
mod streaming_tests {
    use super::{Sha256, sha256};

    fn lower_hex(bytes: &[u8]) -> String {
        use std::fmt::Write;

        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
        }
        output
    }

    #[test]
    fn known_sha256_vectors_match() {
        assert_eq!(
            lower_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            lower_hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn incremental_boundaries_match_one_shot_hashing() {
        let input: Vec<_> = (0_u8..=255).cycle().take(4097).collect();
        let expected = sha256(&input);
        for chunk_size in [1, 7, 63, 64, 65, 511] {
            let mut digest = Sha256::new();
            for chunk in input.chunks(chunk_size) {
                digest.update(chunk);
            }
            assert_eq!(digest.finish(), expected, "chunk size {chunk_size}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sha256;

    #[test]
    fn matches_published_empty_and_abc_vectors() {
        assert_eq!(
            sha256(b""),
            [
                0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                0x78, 0x52, 0xb8, 0x55,
            ]
        );
        assert_eq!(
            sha256(b"abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad,
            ]
        );
    }
}
