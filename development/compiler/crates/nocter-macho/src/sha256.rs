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

pub(crate) fn digest(input: &[u8]) -> [u8; 32] {
    let bit_length = u64::try_from(input.len())
        .expect("slice length fits u64")
        .wrapping_mul(8);
    let padding = 1 + ((55_usize.wrapping_sub(input.len())) & 63) + 8;
    let mut message = Vec::with_capacity(input.len() + padding);
    message.extend_from_slice(input);
    message.push(0x80);
    message.resize(message.len() + padding - 9, 0);
    message.extend_from_slice(&bit_length.to_be_bytes());

    let mut state = INITIAL;
    for block in message.chunks_exact(64) {
        compress(&mut state, block);
    }

    let mut output = [0_u8; 32];
    for (destination, word) in output.chunks_exact_mut(4).zip(state) {
        destination.copy_from_slice(&word.to_be_bytes());
    }
    output
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
