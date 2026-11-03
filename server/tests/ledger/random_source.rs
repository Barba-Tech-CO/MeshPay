/// Tiny xorshift64* generator, so the random tests need no new dependency and replay from a seed.
pub struct RandomSource(u64);

impl RandomSource {
  /// Seed from env `name` when set (to replay a failure), else from the OS.
  pub fn from_env_or_os(name: &str) -> (Self, u64) {
    let seed = std::env::var(name)
      .ok()
      .and_then(|value| value.parse::<u64>().ok())
      .unwrap_or_else(|| {
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes).unwrap();
        u64::from_le_bytes(bytes)
      });
    (Self(seed | 1), seed)
  }

  pub fn next(&mut self) -> u64 {
    self.0 ^= self.0 >> 12;
    self.0 ^= self.0 << 25;
    self.0 ^= self.0 >> 27;
    self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
  }

  /// Uniform-enough value in `0..bound`.
  pub fn below(&mut self, bound: u64) -> u64 {
    self.next() % bound
  }
}
