//! Seeded single-pass reservoir sampling (Vitter's Algorithm R).
//!
//! The RNG is SplitMix64 and a draw in `[0, n)` is Lemire's multiply-shift
//! `(next_u64() * n) >> 64` (no rejection step). Both are specified here
//! exactly so that any implementation (in any language) can reproduce the same
//! sample from the same dump, seed and N: the k-th eligible record (0-based)
//! is kept unconditionally while `k < N`; otherwise one draw `j = below(k+1)`
//! is made and the record replaces slot `j` iff `j < N`.

#[derive(Clone, Debug)]
/// SplitMix64 (Steele, Lea and Flood 2014): a 64-bit generator with a
/// one-word state, specified exactly so samples can be reproduced.
///
/// ```
/// let mut g = yorc::dump::SplitMix64::new(0);
/// assert_eq!(g.next_u64(), 0xe220_a839_7b1d_cdaf);
/// assert!(g.below(10) < 10);
/// ```
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator whose state starts at `seed`.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next 64-bit output.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Value in `[0, n)`; `n` must be > 0.
    pub fn below(&mut self, n: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }
}

/// What to do with the item just offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Keep it at the end (the reservoir is not full yet).
    Append,
    /// Keep it in place of the item at this index.
    Replace(usize),
    /// Drop it.
    Skip,
}

/// A fixed-size uniform sample of a stream (Algorithm R), seeded and
/// reproducible: see the module documentation for the exact rule.
///
/// ```
/// use yorc::dump::{Reservoir, Slot};
///
/// let mut r = Reservoir::new(3, 42);
/// for id in 0..1000u32 {
///     let slot = r.offer();
///     if slot != Slot::Skip {
///         r.put(slot, id); // read the item only when it is kept
///     }
/// }
/// assert_eq!(r.seen(), 1000);
/// let sample = r.into_items();
/// assert_eq!(sample.len(), 3);
///
/// // the same seed and stream give the same sample
/// let mut again = Reservoir::new(3, 42);
/// for id in 0..1000u32 {
///     let slot = again.offer();
///     again.put(slot, id);
/// }
/// assert_eq!(again.into_items(), sample);
/// ```
pub struct Reservoir<T> {
    cap: usize,
    seen: u64,
    items: Vec<T>,
    rng: SplitMix64,
}

impl<T> Reservoir<T> {
    /// A reservoir keeping at most `cap` items, drawing with `SplitMix64(seed)`.
    pub fn new(cap: usize, seed: u64) -> Self {
        Self {
            cap,
            seen: 0,
            items: Vec::with_capacity(cap.min(1 << 20)),
            rng: SplitMix64::new(seed),
        }
    }

    /// Decide the fate of the next eligible item. Call exactly once per
    /// eligible item, in stream order; the item's content is only needed if
    /// the result is not [`Slot::Skip`], so callers can avoid reading it.
    pub fn offer(&mut self) -> Slot {
        let k = self.seen;
        self.seen += 1;
        if self.cap == 0 {
            return Slot::Skip;
        }
        if k < self.cap as u64 {
            return Slot::Append;
        }
        let j = self.rng.below(k + 1);
        if j < self.cap as u64 {
            Slot::Replace(j as usize)
        } else {
            Slot::Skip
        }
    }

    /// Stores `item` as [`offer`](Self::offer) decided.
    pub fn put(&mut self, slot: Slot, item: T) {
        match slot {
            Slot::Append => self.items.push(item),
            Slot::Replace(j) => self.items[j] = item,
            Slot::Skip => {}
        }
    }

    /// Number of items offered so far.
    pub fn seen(&self) -> u64 {
        self.seen
    }

    /// The sample, in slot order.
    pub fn into_items(self) -> Vec<T> {
        self.items
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: usize, seed: u64, total: u32) -> Vec<u32> {
        let mut r = Reservoir::new(n, seed);
        for i in 0..total {
            let slot = r.offer();
            r.put(slot, i);
        }
        r.into_items()
    }

    #[test]
    fn splitmix64_reference_values() {
        let mut g = SplitMix64::new(0);
        assert_eq!(g.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(g.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        let mut g = SplitMix64::new(1_234_567);
        assert_eq!(g.next_u64(), 6_457_827_717_110_365_317);
        assert_eq!(g.next_u64(), 3_203_168_211_198_807_973);
        assert_eq!(g.next_u64(), 9_817_491_932_198_370_423);
    }

    #[test]
    fn below_is_in_range() {
        let mut g = SplitMix64::new(42);
        for n in [1u64, 2, 3, 10, 1000, u64::MAX] {
            for _ in 0..1000 {
                assert!(g.below(n) < n);
            }
        }
    }

    #[test]
    fn keeps_everything_when_stream_is_short() {
        assert_eq!(sample(10, 1, 5), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn zero_capacity() {
        assert!(sample(0, 1, 100).is_empty());
    }

    #[test]
    fn deterministic_per_seed() {
        let a = sample(5, 7, 10_000);
        let b = sample(5, 7, 10_000);
        assert_eq!(a, b);
        assert_eq!(a.len(), 5);
        let c = sample(5, 8, 10_000);
        assert_ne!(a, c);
    }

    #[test]
    fn roughly_uniform() {
        // P(item i kept) = n/total; check over many seeds.
        let (n, total, trials) = (2usize, 10u32, 20_000u64);
        let mut counts = [0u32; 10];
        for seed in 0..trials {
            for x in sample(n, seed, total) {
                counts[x as usize] += 1;
            }
        }
        let expected = trials as f64 * n as f64 / f64::from(total);
        for (i, &c) in counts.iter().enumerate() {
            let dev = (f64::from(c) - expected).abs() / expected;
            assert!(dev < 0.05, "item {i}: {c} vs {expected}");
        }
    }
}
