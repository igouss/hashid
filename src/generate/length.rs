//! Choosing how much entropy an ID needs.

use crate::domain::config::IdConfig;

/// The shortest hash length whose collision probability, at this `population`,
/// stays under the configured bound.
///
/// This is the birthday problem. With a base36 hash of `len` characters there
/// are `36^len` distinct values, and drawing `n` of them at random collides
/// with probability roughly
///
/// ```text
/// P(collision) ≈ 1 - e^(-n² / 2·36^len)
/// ```
///
/// We walk `min..=max` and return the first length that brings `P` under
/// [`IdConfig::max_collision_prob`], falling back to `max` if none does. At the
/// defaults (3–8 chars, 25% bound) that means a young dataset gets 3-character
/// IDs a human can retype, and a dataset of a million records has silently
/// widened to 8.
///
/// A non-trivial collision probability is *fine* here, and that is the design:
/// the generator checks every candidate against the registry anyway. This
/// function only decides how often that check has to do real work — it is not
/// the thing that guarantees uniqueness.
#[allow(
    clippy::cast_precision_loss,
    reason = "population is an approximate input to a probability estimate; f64 \
              carries 2^53 exactly, far beyond any real dataset"
)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "hash lengths are bounded by MAX_HASH_LEN (40), so the i32 cast for \
              powi cannot truncate or wrap"
)]
pub(crate) fn optimal_length(config: &IdConfig, population: usize) -> usize {
    let n: f64 = population as f64;

    for length in config.min_hash_length()..=config.max_hash_length() {
        let space: f64 = 36_f64.powi(length as i32);
        let collision_prob: f64 = 1.0 - (-n * n / (2.0 * space)).exp();
        if collision_prob < config.max_collision_prob() {
            return length;
        }
    }

    config.max_hash_length()
}
