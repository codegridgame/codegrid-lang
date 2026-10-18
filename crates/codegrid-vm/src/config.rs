/// Explicit host-supplied settings that affect normative VM execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmConfig {
    seed: u64,
}

impl VmConfig {
    pub const fn new(seed: u64) -> Self {
        Self { seed }
    }

    pub const fn seed(self) -> u64 {
        self.seed
    }
}

#[cfg(test)]
mod tests {
    use super::VmConfig;
    #[test]
    fn retains_explicit_seed() {
        let config = VmConfig::new(u64::MAX);

        assert_eq!(config.seed(), u64::MAX);
    }
}
