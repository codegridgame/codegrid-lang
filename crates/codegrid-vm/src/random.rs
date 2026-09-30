use codegrid_model::{Direction, Slot};

const OUTER_DOMAIN: u64 = 0x4347_4F55_5445_5231;
const CUSTOM_DOMAIN: u64 = 0x4347_4355_5354_4F4D;
const INTERNAL_DOMAIN: u64 = 0x4347_494E_5445_5231;

/// Applies the CodeGrid v2 SplitMix64 mixing function.
pub const fn mix64(value: u64) -> u64 {
    let mut mixed = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^ (mixed >> 31)
}

/// Derives the initial PRNG state for an outer thread.
pub const fn outer_thread_state(vm_seed: u64, thread_id: u64) -> u64 {
    mix64(mix64(vm_seed ^ OUTER_DOMAIN) ^ thread_id)
}

/// Derives the deterministic seed for one outer Custom invocation.
///
/// `global_tick` is one-based, including a tick whose execution later rolls
/// back.
pub const fn custom_invocation_seed(
    vm_seed: u64,
    caller_thread_id: u64,
    global_tick: u64,
    custom_id: Slot,
) -> u64 {
    mix64(
        mix64(mix64(mix64(vm_seed ^ CUSTOM_DOMAIN) ^ caller_thread_id) ^ global_tick)
            ^ custom_id.get() as u64,
    )
}

/// Derives the initial PRNG state for an internal Custom thread.
pub const fn internal_thread_state(invocation_seed: u64, thread_id: u64) -> u64 {
    mix64(mix64(invocation_seed ^ INTERNAL_DOMAIN) ^ thread_id)
}

/// A private SplitMix64 stream owned by one VM thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(state: u64) -> Self {
        Self { state }
    }

    pub const fn state(self) -> u64 {
        self.state
    }

    /// Advances this stream once and returns the next deterministic value.
    pub fn next_u64(&mut self) -> u64 {
        self.state = mix64(self.state);
        self.state
    }

    /// Advances once and maps the two low bits to a CodeGrid direction.
    pub fn next_direction(&mut self) -> Direction {
        match self.next_u64() & 3 {
            0 => Direction::Up,
            1 => Direction::Down,
            2 => Direction::Left,
            _ => Direction::Right,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{custom_invocation_seed, internal_thread_state, outer_thread_state, SplitMix64};
    use codegrid_model::{Direction, Slot};

    #[test]
    fn matches_outer_golden_vectors() {
        let mut first = SplitMix64::new(outer_thread_state(0, 0));
        assert_eq!(first.state(), 0x034C_A5DF_E1C6_5CBB);
        assert_eq!(first.next_u64(), 0x531E_83D8_554C_75ED);
        assert_eq!(
            SplitMix64::new(outer_thread_state(0, 0)).next_direction(),
            Direction::Down
        );

        let mut second = SplitMix64::new(outer_thread_state(0, 1));
        assert_eq!(second.state(), 0xE655_A6A9_5CAF_D120);
        assert_eq!(second.next_u64(), 0xF21C_3460_065E_08B3);
        assert_eq!(
            SplitMix64::new(outer_thread_state(0, 1)).next_direction(),
            Direction::Right
        );

        let mut third = SplitMix64::new(outer_thread_state(1, 0));
        assert_eq!(third.state(), 0xAEE0_C8FD_2B96_43C3);
        assert_eq!(third.next_u64(), 0xFB39_66F2_7831_B1E1);
        assert_eq!(
            SplitMix64::new(outer_thread_state(1, 0)).next_direction(),
            Direction::Down
        );
    }

    #[test]
    fn matches_custom_golden_vector() {
        let custom_id = Slot::new(0).expect("zero is a valid Custom ID");
        let invocation = custom_invocation_seed(0, 0, 1, custom_id);
        assert_eq!(invocation, 0xC41E_47EA_B76E_9229);

        let mut internal = SplitMix64::new(internal_thread_state(invocation, 0));
        assert_eq!(internal.state(), 0x4C7F_D3BC_90D8_6658);
        assert_eq!(internal.next_u64(), 0xD497_36B3_57C0_DA75);
        assert_eq!(
            SplitMix64::new(internal_thread_state(invocation, 0)).next_direction(),
            Direction::Down
        );
    }

    #[test]
    fn thread_streams_are_independent_values() {
        let mut first = SplitMix64::new(outer_thread_state(17, 2));
        let mut second = SplitMix64::new(outer_thread_state(17, 3));
        let first_value = first.next_u64();
        let second_value = second.next_u64();

        assert_ne!(first_value, second_value);
        assert_eq!(first.state(), first_value);
        assert_eq!(second.state(), second_value);
    }
}
