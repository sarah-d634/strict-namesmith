//! A small, dependency-free pseudo-random number generator.
//!
//! This is SplitMix64. It is not cryptographically secure and is not meant
//! to be: it exists so `NameGenerator` can pick list entries without pulling
//! in an external `rand` dependency.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static ENTROPY_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct Rng {
    state: u64,
}

impl Rng {
    /// Creates a generator with a fixed, reproducible seed. Two `Rng`s built
    /// from the same seed produce the same sequence of values.
    pub fn from_seed(seed: u64) -> Self {
        Rng { state: seed }
    }

    /// Creates a generator seeded from wall-clock time mixed with a
    /// process-local counter, so two generators created in the same nanosecond
    /// still diverge. The seed is guessable and this must not be used for
    /// anything security-sensitive.
    pub fn from_entropy() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let count = ENTROPY_COUNTER.fetch_add(1, Ordering::Relaxed);
        Rng::from_seed(nanos ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }

    /// Advances the state and returns the next 64 bits of output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Returns a value in `0..bound`, unbiased, using Lemire's rejection
    /// method. Panics if `bound` is zero.
    pub fn below(&mut self, bound: usize) -> usize {
        assert!(bound > 0, "below() called with a zero bound");
        let bound = bound as u64;
        let mut x = self.next_u64();
        let mut wide = (x as u128) * (bound as u128);
        let mut low = wide as u64;
        if low < bound {
            let threshold = bound.wrapping_neg() % bound;
            while low < threshold {
                x = self.next_u64();
                wide = (x as u128) * (bound as u128);
                low = wide as u64;
            }
        }
        (wide >> 64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Rng::from_seed(7);
        for _ in 0..1000 {
            assert!(rng.below(5) < 5);
        }
    }
}
