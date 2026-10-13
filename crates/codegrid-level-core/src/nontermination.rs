//! Early non-termination detection for logical level evaluation.
//!
//! Toroidal boards make accidental infinite loops common. Waiting for the
//! per-test tick ceiling wastes the full budget on every such run, so the
//! evaluator samples semantic case state at committed tick boundaries and
//! stops early on two conditions:
//!
//! - [`NonTerminationReason::StateCycle`]: an exact semantic state repeats at
//!   two committed ticks. State comparison is exact (`Eq`), never hashed, so a
//!   report is sound: the case is deterministic from that state onward and can
//!   never make further progress. Sampling covers everything that influences
//!   future transitions and folds in the monotone goal-progress counter, so a
//!   repeat also proves that no progress was made during the cycle.
//! - [`NonTerminationReason::NoProgress`]: no goal progress occurred for a
//!   host-configured consecutive committed-tick window. This check is a
//!   heuristic and is disabled unless the host supplies a window.
//!
//! Observations are deduplicated by committed tick: budget slicing and pending
//! output consumption can re-enter a sampling point without advancing the VM,
//! and those repeats must not be mistaken for a cycle.
use crate::result::NonTerminationReason;
use codegrid_ir::{ScopedProgram, VerifiedProgram};
use codegrid_model::{PrimaryInstruction, Value};
use codegrid_vm::{Memory, ThreadSnapshot, VmSnapshot, VmStatus};
use std::num::NonZeroU64;

use crate::schema::ProgramRules;
use crate::validate::validate_primary;

/// The future-determining projection of VM state at a committed tick boundary.
///
/// Monotone accumulators that never influence later transitions are excluded:
/// committed ticks, metrics, accumulated output, terminal errors and faults.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmSemanticState {
    status: VmStatus,
    registers: [Value; 10],
    memory: Memory,
    input: Vec<Value>,
    runtime_program: ScopedProgram,
    threads: Vec<ThreadSnapshot>,
}
impl VmSemanticState {
    pub fn from_snapshot(snapshot: &VmSnapshot) -> Self {
        Self {
            status: snapshot.status,
            registers: snapshot.registers.clone(),
            memory: snapshot.memory.clone(),
            input: snapshot.input.clone(),
            runtime_program: snapshot.runtime_program.clone(),
            threads: snapshot.threads.clone(),
        }
    }
}

/// The full observable case state compared for exact cycle detection.
///
/// `machine` is the deterministic scene-machine semantic encoding, empty on
/// the ExactIO path. `progress` is the monotone goal-progress counter; keeping
/// it in the compared key makes a reported cycle also prove that no progress
/// was made between the two observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseState {
    pub vm: VmSemanticState,
    pub machine: Vec<u8>,
    pub progress: u64,
}

/// Brent cycle finding over an observed state stream with exact comparison.
///
/// Constant memory: one checkpoint key plus counters. Any equality between two
/// observations proves a repeat; the power-of-two checkpointing only bounds
/// detection latency after a cycle begins.
pub struct CycleDetector<K> {
    checkpoint: Option<K>,
    power: u64,
    lam: u64,
    last_tick: Option<u64>,
}
impl<K: Clone + Eq> CycleDetector<K> {
    pub fn new() -> Self {
        Self {
            checkpoint: None,
            power: 1,
            lam: 0,
            last_tick: None,
        }
    }

    /// Observe the case state at one committed tick boundary.
    ///
    /// Repeated observations of the same committed tick are ignored so that
    /// budget slicing and pending output consumption cannot fake a cycle.
    pub fn observe(&mut self, tick: u64, key: &K) -> Option<NonTerminationReason> {
        if self.last_tick.is_some_and(|last| tick <= last) {
            return None;
        }
        self.last_tick = Some(tick);
        match self.checkpoint.as_ref() {
            None => {
                self.checkpoint = Some(key.clone());
                self.power = 1;
                self.lam = 0;
                None
            }
            Some(checkpoint) => {
                if checkpoint == key {
                    return Some(NonTerminationReason::StateCycle);
                }
                self.lam += 1;
                if self.lam == self.power {
                    self.checkpoint = Some(key.clone());
                    self.power = self.power.saturating_mul(2);
                    self.lam = 0;
                }
                None
            }
        }
    }
}
impl<K: Clone + Eq> Default for CycleDetector<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// Heuristic stall detection over a monotone goal-progress counter.
///
/// Fires when `progress` fails to grow for `window` consecutive committed
/// ticks. Disabled unless a host supplies a window.
pub struct NoProgressTracker {
    window: NonZeroU64,
    last_progress_tick: u64,
    last_progress: u64,
    last_tick: Option<u64>,
}
impl NoProgressTracker {
    pub fn new(window: NonZeroU64) -> Self {
        Self {
            window,
            last_progress_tick: 0,
            last_progress: 0,
            last_tick: None,
        }
    }

    /// Observe the progress counter at one committed tick boundary.
    pub fn observe(&mut self, tick: u64, progress: u64) -> Option<NonTerminationReason> {
        if self.last_tick.is_some_and(|last| tick <= last) {
            return None;
        }
        self.last_tick = Some(tick);
        if progress > self.last_progress {
            self.last_progress = progress;
            self.last_progress_tick = tick;
            return None;
        }
        if tick - self.last_progress_tick >= self.window.get() {
            Some(NonTerminationReason::NoProgress)
        } else {
            None
        }
    }
}

/// Exact cycle detection is sound only when case semantics are Markovian in
/// the sampled state.
///
/// Custom invocations reseed their thread PRNG from the global tick
/// (`codegrid_vm::custom_invocation_seed`), so a program that both defines
/// customs and may execute `RANDOM_DIRECTION` derives future behavior from the
/// absolute tick and is excluded. Without customs no such reseeding exists;
/// with `RANDOM_DIRECTION` rejected the PRNG never influences behavior.
pub fn exact_cycle_detection_eligible(program: &VerifiedProgram, rules: &ProgramRules) -> bool {
    program.program().customs.is_empty()
        || validate_primary(rules, Some(PrimaryInstruction::RandomDirection), true).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use codegrid_ir::Board;
    use codegrid_vm::RuntimeMetrics;
    use std::collections::BTreeMap;

    #[test]
    fn constant_stream_reports_state_cycle_on_first_repeat() {
        let mut detector = CycleDetector::new();
        assert_eq!(detector.observe(1, &"a"), None);
        assert_eq!(
            detector.observe(2, &"a"),
            Some(NonTerminationReason::StateCycle)
        );
    }

    #[test]
    fn alternating_stream_reports_state_cycle() {
        let mut detector = CycleDetector::new();
        let mut found = None;
        for tick in 1..=8u64 {
            let key = tick % 2;
            if let Some(reason) = detector.observe(tick, &key) {
                found = Some((tick, reason));
                break;
            }
        }
        assert_eq!(found, Some((4, NonTerminationReason::StateCycle)));
    }

    #[test]
    fn longer_cycle_is_detected_within_a_bounded_window() {
        let mut detector = CycleDetector::new();
        let mut found = None;
        for tick in 1..=32u64 {
            if let Some(reason) = detector.observe(tick, &(tick % 3)) {
                found = Some(reason);
                break;
            }
        }
        assert_eq!(found, Some(NonTerminationReason::StateCycle));
    }

    #[test]
    fn strictly_changing_stream_reports_nothing() {
        let mut detector = CycleDetector::new();
        for tick in 1..=100u64 {
            assert_eq!(detector.observe(tick, &tick), None);
        }
    }

    #[test]
    fn duplicate_committed_tick_observations_are_ignored() {
        let mut detector = CycleDetector::new();
        assert_eq!(detector.observe(1, &7), None);
        // A re-entry at the same committed tick must not count as a repeat.
        assert_eq!(detector.observe(1, &7), None);
        assert_eq!(detector.observe(2, &8), None);
        assert_eq!(detector.observe(2, &7), None);
        assert_eq!(detector.observe(3, &7), None);
        assert_eq!(
            detector.observe(4, &8),
            Some(NonTerminationReason::StateCycle)
        );
    }

    #[test]
    fn no_progress_window_fires_and_resets_on_growth() {
        let mut tracker = NoProgressTracker::new(NonZeroU64::new(3).unwrap());
        assert_eq!(tracker.observe(1, 0), None);
        assert_eq!(tracker.observe(2, 0), None);
        assert_eq!(
            tracker.observe(3, 0),
            Some(NonTerminationReason::NoProgress)
        );

        let mut tracker = NoProgressTracker::new(NonZeroU64::new(3).unwrap());
        assert_eq!(tracker.observe(1, 1), None);
        assert_eq!(tracker.observe(2, 1), None);
        assert_eq!(tracker.observe(3, 2), None);
        assert_eq!(tracker.observe(4, 2), None);
        assert_eq!(tracker.observe(5, 2), None);
        assert_eq!(
            tracker.observe(6, 2),
            Some(NonTerminationReason::NoProgress)
        );
    }

    #[test]
    fn no_progress_ignores_duplicate_ticks() {
        let mut tracker = NoProgressTracker::new(NonZeroU64::new(2).unwrap());
        assert_eq!(tracker.observe(1, 0), None);
        assert_eq!(tracker.observe(1, 0), None);
        assert_eq!(
            tracker.observe(2, 0),
            Some(NonTerminationReason::NoProgress)
        );
    }

    fn snapshot() -> VmSnapshot {
        VmSnapshot {
            status: VmStatus::Running,
            committed_ticks: 5,
            registers: [0; 10],
            memory: Memory::default(),
            input: Vec::new(),
            output: vec![1, 2],
            runtime_program: ScopedProgram {
                main: Board {
                    width: 1,
                    height: 1,
                    cells: Vec::new(),
                    folded_blocks: BTreeMap::new(),
                },
                functions: BTreeMap::new(),
            },
            threads: Vec::new(),
            metrics: RuntimeMetrics::default(),
            errors: Vec::new(),
            fault: None,
        }
    }

    #[test]
    fn semantic_state_ignores_monotone_fields_but_tracks_future_state() {
        let base = VmSemanticState::from_snapshot(&snapshot());
        let mut drifted = snapshot();
        drifted.committed_ticks = 9;
        drifted.output = vec![1, 2, 3, 4];
        assert_eq!(base, VmSemanticState::from_snapshot(&drifted));
        let mut changed = snapshot();
        changed.registers[0] = 1;
        assert_ne!(base, VmSemanticState::from_snapshot(&changed));
        let mut queued = snapshot();
        queued.input = vec![7];
        assert_ne!(base, VmSemanticState::from_snapshot(&queued));
    }
}
