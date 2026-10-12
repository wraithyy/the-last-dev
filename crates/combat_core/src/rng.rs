/// SplitMix64: tiny, fast and fully specified, so replays are identical on
/// every platform and no `rand` dependency (whose streams may change across
/// versions) is needed.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`; modulo bias is negligible for the small `n` used here.
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % u64::from(n)) as u32
    }

    /// Uniform in `lo..=hi` (swapped bounds are tolerated).
    pub fn range_inclusive(&mut self, lo: u32, hi: u32) -> u32 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        lo + self.below(hi - lo + 1)
    }

    /// True with probability `pct` percent.
    pub fn chance_pct(&mut self, pct: u32) -> bool {
        self.below(100) < pct
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let (mut a, mut b) = (Rng::new(42), Rng::new(42));
        assert!((0..100).all(|_| a.next_u64() == b.next_u64()));
    }

    #[test]
    fn known_first_value_pins_the_algorithm() {
        // Reference SplitMix64 output for seed 0.
        assert_eq!(Rng::new(0).next_u64(), 0xE220_A839_7B1D_CDAF);
    }

    #[test]
    fn range_is_inclusive_and_bounded() {
        let mut r = Rng::new(7);
        let v: Vec<u32> = (0..500).map(|_| r.range_inclusive(8000, 8003)).collect();
        assert!(v.iter().all(|x| (8000..=8003).contains(x)));
        assert!(v.contains(&8000) && v.contains(&8003));
    }
}
