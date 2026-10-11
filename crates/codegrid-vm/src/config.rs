use std::num::NonZeroU64;

pub const DEFAULT_GAS_HARD_LIMIT: u64 = 100_000_000;

/// Explicit host-supplied settings that affect normative VM execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmConfig {
    #[cfg(test)]
    allow_custom_for_test: bool,
    seed: u64,
    gas_hard_limit: NonZeroU64,
    custom_execution_limit: NonZeroU64,
}

impl VmConfig {
    pub const fn new(seed: u64, custom_execution_limit: NonZeroU64) -> Self {
        Self {
            #[cfg(test)]
            allow_custom_for_test: false,
            seed,
            gas_hard_limit: NonZeroU64::new(DEFAULT_GAS_HARD_LIMIT).unwrap(),
            custom_execution_limit,
        }
    }

    pub(crate) const fn custom_execution_enabled(self) -> bool {
        #[cfg(test)]
        {
            self.allow_custom_for_test
        }
        #[cfg(not(test))]
        {
            false
        }
    }

    #[cfg(test)]
    pub(crate) const fn with_custom_execution_for_test(mut self) -> Self {
        self.allow_custom_for_test = true;
        self
    }

    pub const fn with_gas_hard_limit(mut self, limit: NonZeroU64) -> Self {
        self.gas_hard_limit = limit;
        self
    }

    pub const fn gas_hard_limit(self) -> NonZeroU64 {
        self.gas_hard_limit
    }

    pub const fn seed(self) -> u64 {
        self.seed
    }

    pub const fn custom_execution_limit(self) -> NonZeroU64 {
        self.custom_execution_limit
    }
}

#[cfg(test)]
mod tests {
    use super::VmConfig;
    use std::num::NonZeroU64;

    #[test]
    fn retains_explicit_seed_and_positive_custom_limit() {
        let config = VmConfig::new(u64::MAX, NonZeroU64::new(42).expect("42 is positive"));

        assert!(!config.custom_execution_enabled());
        assert_eq!(config.seed(), u64::MAX);
        assert_eq!(config.custom_execution_limit().get(), 42);
        assert!(NonZeroU64::new(0).is_none());
    }
}
