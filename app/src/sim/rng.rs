//! The simulator's random numbers: Prando's xorshift32, exactly as Moomoolator uses it
//! (`SeededRng` in uma-skill-tools/Random.ts), so a seed gives the same stream of draws.

const MIN: f64 = -2_147_483_648.0;
const MAX: f64 = 2_147_483_647.0;

/// JavaScript's ToInt32: truncate, wrap modulo 2^32, reinterpret as signed.
fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    x.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
}

#[derive(Clone, Debug)]
pub struct Rng {
    value: i32,
}

impl Rng {
    pub fn new(seed: f64) -> Self {
        // Prando's getSafeSeed maps 0 to 1 before the first xorshift converts it to an int32.
        Rng {
            value: to_int32(if seed == 0.0 { 1.0 } else { seed }),
        }
    }

    fn recalculate(&mut self) {
        let mut v = self.value;
        v ^= v.wrapping_shl(13);
        v ^= v >> 17;
        v ^= v.wrapping_shl(5);
        self.value = v;
    }

    /// Uniform in [0, 1] (Prando's `next()`).
    pub fn random(&mut self) -> f64 {
        self.recalculate();
        (f64::from(self.value) - MIN) / (MAX - MIN)
    }

    /// `floor(r * upper)` for r in [0, 1): Prando's `nextInt(0, upper - 1)`. Like the original,
    /// `upper` may be fractional or negative.
    pub fn uniform(&mut self, upper: f64) -> f64 {
        self.recalculate();
        ((f64::from(self.value) - MIN) / (MAX + 1.0 - MIN) * upper).floor()
    }

    /// A seed for a child generator, 0..=2^32.
    pub fn int32(&mut self) -> f64 {
        (self.random() * 4_294_967_296.0).floor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xorshift_matches_prando() {
        // Prando(1): xorshift32 of 1 is 270369, mapped from [-2^31, 2^31-1] to [0, 1].
        let mut r = Rng::new(1.0);
        let expected = (270_369.0 - MIN) / (MAX - MIN);
        assert!((r.random() - expected).abs() < 1e-15);
        // Seeds wrap like JavaScript's int32 conversion.
        let (mut a, mut b) = (Rng::new(4_294_967_297.0), Rng::new(1.0));
        assert_eq!(a.random(), b.random());
        let mut c = Rng::new(42.0);
        for _ in 0..1000 {
            let u = c.uniform(10.0);
            assert!((0.0..10.0).contains(&u) && u.fract() == 0.0);
        }
    }
}
