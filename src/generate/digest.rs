//! Turning hashed bytes into a short, typeable base36 string.

use sha2::{Digest as _, Sha256};

use crate::domain::hash::Base36Hash;

/// The base36 alphabet: digits then lowercase letters, in ASCII order.
const ALPHABET: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// How many bytes of the SHA-256 digest feed the base36 encoding.
///
/// Eight bytes is exactly a `u64`, which caps the encoded form at 13 base36
/// characters — comfortably above any hash length the grammar allows.
const DIGEST_BYTES: usize = 8;

/// Hash `input` and render it as exactly `length` base36 characters.
///
/// SHA-256, then the leading `DIGEST_BYTES` bytes as a big-endian `u64`, then
/// base36, then the **last** `length` characters — not the first. Truncating
/// from the tail keeps the least-significant digits, which vary fastest; taking
/// the head would throw away most of the entropy for short lengths, since the
/// leading digits of a `u64` barely move.
///
/// Short encodings are left-padded with `0` so the output length is exactly
/// `length` for any input.
///
/// # Panics
///
/// Never: every byte the encoder emits is drawn from `ALPHABET`, so the
/// result is base36 by construction and its `Base36Hash::parse` cannot fail.
#[must_use]
pub fn digest(input: &str, length: usize) -> Base36Hash {
    let mut hasher: Sha256 = Sha256::new();
    hasher.update(input.as_bytes());
    let bytes = hasher.finalize();

    let mut packed: u64 = 0;
    for &byte in bytes.iter().take(DIGEST_BYTES) {
        packed = (packed << 8) | u64::from(byte);
    }

    let mut encoded: String = base36(packed);
    if encoded.len() < length {
        encoded = format!("{encoded:0>length$}");
    }

    let tail: String = encoded
        .chars()
        .skip(encoded.len().saturating_sub(length))
        .collect();

    Base36Hash::parse(&tail)
        .unwrap_or_else(|_| unreachable!("base36 encoder emits only [0-9a-z] characters"))
}

/// Render `num` in base36, lowercase, without padding.
#[must_use]
pub fn base36(mut num: u64) -> String {
    if num == 0 {
        return "0".to_owned();
    }
    let mut reversed: Vec<char> = Vec::new();
    while num > 0 {
        let index: usize = usize::try_from(num % 36).unwrap_or(0);
        reversed.push(char::from(ALPHABET[index]));
        num /= 36;
    }
    reversed.into_iter().rev().collect()
}
