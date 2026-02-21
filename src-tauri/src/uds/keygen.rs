/// KeyGenMkI — Security Access key generation algorithm.
/// Ported verbatim from the Python implementation in try_603e_wait.py.
///
/// # Arguments
/// * `seed` - 3-byte seed from ECU (as u32, only lower 24 bits used)
/// * `constants` - 5-byte security constants (e.g., DC0314: [0x65, 0xF8, 0x24, 0xAC, 0x8F])
///
/// # Returns
/// 3-byte key as u32 (lower 24 bits)
pub fn keygen_mki(seed: u32, constants: &[u8; 5]) -> u32 {
    let sknum = constants[0] as u32;
    let sknum2 = constants[1] as u32;
    let sknum3 = constants[2] as u32;
    let sknum4 = constants[3] as u32;
    let sknum5 = constants[4] as u32;

    let sknum13 = (seed >> 0x10) & 0xFF;
    let b2 = (seed >> 8) & 0xFF;
    let b3 = seed & 0xFF;
    let sknum6 = (sknum13 << 0x10) + (b2 << 8) + b3;

    let sknum7 = ((sknum6 & 0xFF0000) >> 0x10)
        | (sknum6 & 0xFF00)
        | (sknum << 0x18)
        | ((sknum6 & 0xFF) << 0x10);

    let mut sknum8: u32 = 0xC541A9;

    // First loop: process seed with sknum7
    for i in 0..0x20u32 {
        let sknum10 = (((sknum7 >> i & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1);
        let old = sknum8 >> 1;
        let hb = (sknum10 & 0x800000) >> 0x17;
        sknum8 = (sknum10 & 0xEF6FD7)
            | (((sknum10 & 0x100000) >> 0x14 ^ hb) << 0x14)
            | (((old & 0x8000) >> 0xF ^ hb) << 0xF)
            | (((old & 0x1000) >> 0xC ^ hb) << 0xC)
            | (0x20 * ((old & 0x20) >> 5 ^ hb))
            | (8 * ((old & 8) >> 3 ^ hb));
    }

    // Second loop: process constants
    let fc = (sknum5 << 0x18) | (sknum4 << 0x10) | sknum2 | (sknum3 << 8);
    for j in 0..0x20u32 {
        let sknum12 = (((fc >> j & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1);
        let old = sknum8 >> 1;
        let hb = (sknum12 & 0x800000) >> 0x17;
        sknum8 = (sknum12 & 0xEF6FD7)
            | (((sknum12 & 0x100000) >> 0x14 ^ hb) << 0x14)
            | (((old & 0x8000) >> 0xF ^ hb) << 0xF)
            | (((old & 0x1000) >> 0xC ^ hb) << 0xC)
            | (0x20 * ((old & 0x20) >> 5 ^ hb))
            | (8 * ((old & 8) >> 3 ^ hb));
    }

    // Final bit rearrangement
    let r = ((sknum8 & 0xF0000) >> 0x10)
        | (0x10 * (sknum8 & 0xF))
        | ((((sknum8 & 0xF00000) >> 0x14) | ((sknum8 & 0xF000) >> 8)) << 8)
        | (((sknum8 & 0xFF0) >> 4) << 0x10);

    r & 0xFFFFFF
}

/// IMC DC0314 security constants
pub const DC0314_CONSTANTS: [u8; 5] = [0x65, 0xF8, 0x24, 0xAC, 0x8F];

#[cfg(test)]
mod tests {
    use super::*;

    // Reference Python implementation for verification
    fn keygen_python(seed_int: u32, constants: &[u8; 5]) -> u32 {
        let sknum = constants[0] as u32;
        let sknum2 = constants[1] as u32;
        let sknum3 = constants[2] as u32;
        let sknum4 = constants[3] as u32;
        let sknum5 = constants[4] as u32;

        let sknum13 = (seed_int >> 0x10) & 0xFF;
        let b2 = (seed_int >> 8) & 0xFF;
        let b3 = seed_int & 0xFF;
        let sknum6 = (sknum13 << 0x10) + (b2 << 8) + b3;
        let sknum7 = ((sknum6 & 0xFF0000) >> 0x10)
            | (sknum6 & 0xFF00)
            | (sknum << 0x18)
            | ((sknum6 & 0xFF) << 0x10);
        let mut sknum8: u32 = 0xC541A9;

        for i in 0..0x20u32 {
            let sknum10 = (((sknum7 >> i & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1);
            let old = sknum8 >> 1;
            let hb = (sknum10 & 0x800000) >> 0x17;
            sknum8 = (sknum10 & 0xEF6FD7)
                | (((sknum10 & 0x100000) >> 0x14 ^ hb) << 0x14)
                | (((old & 0x8000) >> 0xF ^ hb) << 0xF)
                | (((old & 0x1000) >> 0xC ^ hb) << 0xC)
                | (0x20 * ((old & 0x20) >> 5 ^ hb))
                | (8 * ((old & 8) >> 3 ^ hb));
        }

        let fc = (sknum5 << 0x18) | (sknum4 << 0x10) | sknum2 | (sknum3 << 8);
        for j in 0..0x20u32 {
            let sknum12 = (((fc >> j & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1);
            let old = sknum8 >> 1;
            let hb = (sknum12 & 0x800000) >> 0x17;
            sknum8 = (sknum12 & 0xEF6FD7)
                | (((sknum12 & 0x100000) >> 0x14 ^ hb) << 0x14)
                | (((old & 0x8000) >> 0xF ^ hb) << 0xF)
                | (((old & 0x1000) >> 0xC ^ hb) << 0xC)
                | (0x20 * ((old & 0x20) >> 5 ^ hb))
                | (8 * ((old & 8) >> 3 ^ hb));
        }

        let r = ((sknum8 & 0xF0000) >> 0x10)
            | (0x10 * (sknum8 & 0xF))
            | ((((sknum8 & 0xF00000) >> 0x14) | ((sknum8 & 0xF000) >> 8)) << 8)
            | (((sknum8 & 0xFF0) >> 4) << 0x10);

        r & 0xFFFFFF
    }

    #[test]
    fn test_keygen_known_seed_1() {
        let key = keygen_mki(0x112233, &DC0314_CONSTANTS);
        let expected = keygen_python(0x112233, &DC0314_CONSTANTS);
        assert_eq!(key, expected);
        // Verify it's a 24-bit value
        assert!(key <= 0xFFFFFF);
    }

    #[test]
    fn test_keygen_known_seed_2() {
        let key = keygen_mki(0xAABBCC, &DC0314_CONSTANTS);
        let expected = keygen_python(0xAABBCC, &DC0314_CONSTANTS);
        assert_eq!(key, expected);
    }

    #[test]
    fn test_keygen_known_seed_3() {
        let key = keygen_mki(0xFFFFFF, &DC0314_CONSTANTS);
        let expected = keygen_python(0xFFFFFF, &DC0314_CONSTANTS);
        assert_eq!(key, expected);
    }

    #[test]
    fn test_keygen_zero_seed() {
        let key = keygen_mki(0x000000, &DC0314_CONSTANTS);
        // Should not panic, should produce valid 24-bit result
        assert!(key <= 0xFFFFFF);
    }

    #[test]
    fn test_keygen_all_ones() {
        let alt_constants: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let key = keygen_mki(0xFFFFFF, &alt_constants);
        let expected = keygen_python(0xFFFFFF, &alt_constants);
        assert_eq!(key, expected);
    }

    #[test]
    fn test_keygen_matches_python() {
        // Test a range of seeds to ensure Rust matches Python exactly
        let test_seeds: Vec<u32> = (0..100)
            .map(|i| {
                // Deterministic pseudo-random seeds
                ((i * 7919 + 104729) % 0xFFFFFF) as u32
            })
            .collect();

        for seed in test_seeds {
            let rust_key = keygen_mki(seed, &DC0314_CONSTANTS);
            let python_key = keygen_python(seed, &DC0314_CONSTANTS);
            assert_eq!(
                rust_key, python_key,
                "Mismatch for seed 0x{:06X}: Rust=0x{:06X}, Python=0x{:06X}",
                seed, rust_key, python_key
            );
        }
    }

    #[test]
    fn test_keygen_result_is_24_bit() {
        // Ensure output is always within 24-bit range
        let seeds = [0x000000, 0x7FFFFF, 0xFFFFFF, 0x123456, 0xABCDEF];
        for seed in seeds {
            let key = keygen_mki(seed, &DC0314_CONSTANTS);
            assert!(
                key <= 0xFFFFFF,
                "Key 0x{:X} exceeds 24-bit for seed 0x{:06X}",
                key,
                seed
            );
        }
    }
}
