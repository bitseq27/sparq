//! A deterministic 64-bit hash for golden-reference comparison (ADR-007).
//!
//! FNV-1a over the raw little-endian bytes of the sample buffer. Chosen deliberately over a
//! cryptographic hash: it is dependency-free, fast, and its only job is to answer "are these two
//! renders bit-identical?". Golden files also record a max-absolute-error comparison, so a hash
//! collision could never hide a real difference.

/// FNV-1a offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Hash a byte slice.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// Hash an `f32` sample buffer by its little-endian byte representation.
///
/// Bit-exact by construction: `-0.0` and `0.0` hash differently, which is what we want when the
/// question is "did the output change at all?".
#[must_use]
pub fn fnv1a64_f32(samples: &[f32]) -> u64 {
    let mut h = FNV_OFFSET;
    for &s in samples {
        for b in s.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(FNV_PRIME);
        }
    }
    h
}

/// Maximum absolute difference between two equal-length buffers, plus the index where it occurs.
///
/// This is the number a golden-test failure must print: "hash differs" is useless on its own.
#[must_use]
pub fn max_abs_diff(a: &[f32], b: &[f32]) -> (f32, usize) {
    let n = a.len().min(b.len());
    let mut max = 0.0f32;
    let mut at = 0usize;
    for i in 0..n {
        let d = (a[i] - b[i]).abs();
        if d > max {
            max = d;
            at = i;
        }
    }
    (max, at)
}

/// Format a hash the way the golden files and CI output do.
#[must_use]
pub fn hex64(h: u64) -> String {
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_fnv1a_vectors() {
        // Canonical FNV-1a 64-bit test vectors.
        assert_eq!(hex64(fnv1a64(b"")), "cbf29ce484222325");
        assert_eq!(hex64(fnv1a64(b"a")), "af63dc4c8601ec8c");
        assert_eq!(hex64(fnv1a64(b"foobar")), "85944171f73967e8");
    }

    #[test]
    fn hash_matches_a_reference_implementation() {
        // Independent, deliberately naive restatement of the algorithm.
        fn reference(bytes: &[u8]) -> u64 {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in bytes {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            h
        }
        for probe in [&b""[..], b"sparq", &[0x00u8, 0xff, 0x10, 0x7f][..], &[7u8; 4096][..]] {
            assert_eq!(fnv1a64(probe), reference(probe));
        }
    }

    #[test]
    fn f32_hash_is_bit_exact() {
        let a = [0.0f32, 1.0, -1.0, 0.5];
        let b = [0.0f32, 1.0, -1.0, 0.5];
        assert_eq!(fnv1a64_f32(&a), fnv1a64_f32(&b));
        let c = [0.0f32, 1.0, -1.0, f32::from_bits(0x3f00_0001)];
        assert_ne!(fnv1a64_f32(&a), fnv1a64_f32(&c), "a 1-ULP change must change the hash");
        assert_ne!(fnv1a64_f32(&[0.0]), fnv1a64_f32(&[-0.0]), "-0.0 and 0.0 are different bits");
    }

    #[test]
    fn max_abs_diff_reports_value_and_index() {
        let a = [0.0f32, 0.1, 0.2, 0.3];
        let b = [0.0f32, 0.1, 0.4, 0.3];
        let (d, i) = max_abs_diff(&a, &b);
        assert_eq!(i, 2);
        assert!((d - 0.2).abs() < 1e-6);
    }
}
