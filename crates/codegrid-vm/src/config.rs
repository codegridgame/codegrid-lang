use std::num::NonZeroU64;

use crate::BoundaryMode;

/// Explicit host-supplied settings that affect normative VM execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VmConfig {
    boundary_mode: BoundaryMode,
    seed: u64,
    custom_execution_limit: NonZeroU64,
}

impl VmConfig {
    pub const fn new(
        boundary_mode: BoundaryMode,
        seed: u64,
        custom_execution_limit: NonZeroU64,
    ) -> Self {
        Self {
            boundary_mode,
            seed,
            custom_execution_limit,
        }
    }

    pub const fn boundary_mode(self) -> BoundaryMode {
        self.boundary_mode
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
    use codegrid_model::BoundaryMode;
    use std::num::NonZeroU64;

    #[test]
    fn retains_explicit_boundary_seed_and_positive_custom_limit() {
        let config = VmConfig::new(
            BoundaryMode::Wrap,
            u64::MAX,
            NonZeroU64::new(42).expect("42 is positive"),
        );

        assert_eq!(config.boundary_mode(), BoundaryMode::Wrap);
        assert_eq!(config.seed(), u64::MAX);
        assert_eq!(config.custom_execution_limit().get(), 42);
        assert!(NonZeroU64::new(0).is_none());
    }
}
