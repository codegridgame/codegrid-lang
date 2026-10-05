//! Host-independent VM building blocks for deterministic CodeGrid execution.
//!
//! This crate accepts only verified IR and explicit runtime data. It contains
//! no file, process, clock, random-device, browser, or game-host access.

mod config;
mod effects;
mod engine;
mod error;
mod memory;
mod metrics;
mod movement;
mod random;
mod state;
mod thread;

pub use codegrid_ir::{TailCallSite, VerifiedProgram};
pub use codegrid_model::BoundaryMode;
pub use config::VmConfig;
pub use engine::{RunOutcome, RunResult, StepResult, VmEvent, WorkLimitExceeded};
pub use error::{ExecutionScope, RuntimeError, RuntimeErrorKind};
pub use memory::{effective_address, Memory, MemoryAddress, Page};
pub use metrics::{
    InstructionKind, MemoryLocationId, MemorySpaceId, MetricCounterOverflow, RuntimeMetricSummary,
    RuntimeMetrics, StaticCellId,
};
pub use movement::{move_folded, move_normal, Coordinate, FoldStep};
pub use random::{
    custom_invocation_seed, internal_thread_state, mix64, outer_thread_state, SplitMix64,
};
pub use state::{
    CallFrameSnapshot, InputAppendError, ThreadPhaseSnapshot, ThreadSnapshot, ThreadSnapshotView,
    Vm, VmFault, VmInitializationError, VmSnapshot, VmSnapshotView, VmStatus,
};
