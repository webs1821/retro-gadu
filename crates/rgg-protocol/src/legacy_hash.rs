// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2024-2026 Oreeeee
// SPDX-FileCopyrightText: 2026 webs1821
// Port of GG32LoginHash from poggadaj, commit 4dc5484621fc53fc3205af80e4f80eca9f1d208d.
// Source: src/internal/security/gg/gg.go. See THIRD_PARTY.toml (poggadaj-gg32).
// Changes: byte-slice input, explicit wrapping arithmetic and rotate_left in Rust.

/// Legacy challenge response; NEVER use this as a password-storage hash.
/// The adapter must encode the password into the correct client character set first.
pub fn gg32(password: &[u8], seed: u32) -> u32 {
    let mut x = 0u32;
    let mut y = seed;
    for &byte in password {
        x = (x & 0xffffff00) | u32::from(byte);
        y ^= x;
        y = y.wrapping_add(x);
        x <<= 8;
        y ^= x;
        x <<= 8;
        y = y.wrapping_sub(x);
        x <<= 8;
        y ^= x;
        y = y.rotate_left(y & 31);
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_synthetic_vectors_cover_zero_rotations_high_bytes_and_overflow() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/gg32.json")).unwrap();
        for row in fixtures.as_array().unwrap() {
            let input: Vec<u8> = row["input"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(
                gg32(&input, row["seed"].as_u64().unwrap() as u32),
                row["output"].as_u64().unwrap() as u32
            );
        }
    }
}
