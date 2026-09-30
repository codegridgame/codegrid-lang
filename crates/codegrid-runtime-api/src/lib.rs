//! Host-neutral, versioned operations for compiling and running CodeGrid.
//!
//! This crate owns no browser, server-runtime, filesystem, process, or game
//! integration. A host creates one `RuntimeApi` per isolation boundary and
//! passes explicit source, input, configuration, and execution budgets.

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use codegrid_compiler::{check as check_source, compile};
pub use codegrid_compiler::{
    BoardView, CellView, CodeGridView, Diagnostic, ProgramView, Severity, Span,
};
pub use codegrid_ir::{BoardId, CodeGridId, Program, ScopedProgram, VerifiedProgram};
pub use codegrid_model::Direction;
pub use codegrid_vm::{
    BoundaryMode, CallFrameSnapshot, Coordinate, ExecutionScope, InstructionKind, MemoryAddress,
    MemoryLocationId, MemorySpaceId, MetricCounterOverflow, Page, RunOutcome, RuntimeError,
    RuntimeErrorKind, RuntimeMetricSummary, RuntimeMetrics, StaticCellId, ThreadPhaseSnapshot,
    VmEvent, VmFault, VmStatus,
};
use codegrid_vm::{
    RunResult as VmRunResult, StepResult as VmStepResult, ThreadSnapshotView, Vm, VmConfig,
    VmInitializationError, VmSnapshotView,
};

/// Version of the host-neutral operation and request/response contract.
pub const RUNTIME_API_VERSION: u32 = 3;

/// Operational safeguards supplied by the embedding host.
///
/// These limits do not alter CodeGrid semantics. Hosts should select values
/// appropriate to their deployment and separately enforce wall-clock, memory,
/// concurrency, and request-size budgets where those facilities exist. A zero
/// count/byte limit disables that capability; per-call and per-instance tick
/// budgets and the per-call work-unit ceiling must be positive so accepted
/// execution slices have finite bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLimits {
    max_source_bytes: usize,
    max_compiled_programs: usize,
    max_instances: usize,
    max_input_bytes: usize,
    max_initial_memory_entries: usize,
    max_run_ticks_per_call: NonZeroU64,
    max_total_ticks_per_instance: NonZeroU64,
    max_work_units_per_call: NonZeroU64,
}

impl HostLimits {
    pub const fn new(
        max_source_bytes: usize,
        max_compiled_programs: usize,
        max_instances: usize,
        max_input_bytes: usize,
        max_initial_memory_entries: usize,
        max_run_ticks_per_call: NonZeroU64,
        max_total_ticks_per_instance: NonZeroU64,
        max_work_units_per_call: NonZeroU64,
    ) -> Self {
        Self {
            max_source_bytes,
            max_compiled_programs,
            max_instances,
            max_input_bytes,
            max_initial_memory_entries,
            max_run_ticks_per_call,
            max_total_ticks_per_instance,
            max_work_units_per_call,
        }
    }

    pub const fn max_source_bytes(self) -> usize {
        self.max_source_bytes
    }

    pub const fn max_compiled_programs(self) -> usize {
        self.max_compiled_programs
    }

    pub const fn max_instances(self) -> usize {
        self.max_instances
    }

    pub const fn max_input_bytes(self) -> usize {
        self.max_input_bytes
    }

    pub const fn max_initial_memory_entries(self) -> usize {
        self.max_initial_memory_entries
    }

    pub const fn max_run_ticks_per_call(self) -> u64 {
        self.max_run_ticks_per_call.get()
    }

    pub const fn max_total_ticks_per_instance(self) -> u64 {
        self.max_total_ticks_per_instance.get()
    }

    pub const fn max_work_units_per_call(self) -> u64 {
        self.max_work_units_per_call.get()
    }

    pub const fn with_max_work_units_per_call(mut self, maximum: NonZeroU64) -> Self {
        self.max_work_units_per_call = maximum;
        self
    }
}

/// Opaque identifier scoped to one `RuntimeApi` instance.
///
/// A WASM adapter must encode this value without lossy JavaScript `Number`
/// conversion, for example as a decimal string or `BigInt`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProgramHandle(u64);

impl ProgramHandle {
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Reconstructs a handle received from an adapter. The registry still
    /// validates that the handle belongs to this runtime instance.
    pub const fn from_raw(value: u64) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }
}

/// Opaque identifier scoped to one `RuntimeApi` instance.
///
/// Handles are monotonically allocated and never reused during the lifetime
/// of a runtime API instance.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InstanceHandle(u64);

impl InstanceHandle {
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Reconstructs a handle received from an adapter. The registry still
    /// validates that the handle belongs to this runtime instance.
    pub const fn from_raw(value: u64) -> Option<Self> {
        if value == 0 {
            None
        } else {
            Some(Self(value))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequest {
    pub api_version: u32,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileResponse {
    pub api_version: u32,
    pub outcome: CompileOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileOutcome {
    Compiled { program: ProgramHandle },
    Diagnostics { items: Vec<Diagnostic> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckRequest {
    pub api_version: u32,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckResponse {
    pub api_version: u32,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramViewRequest {
    pub api_version: u32,
    pub program: ProgramHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramViewResponse {
    pub api_version: u32,
    pub view: ProgramView,
}

/// Borrowed access to immutable, verifier-produced program data.
pub struct ProgramViewRef<'a> {
    inner: &'a VerifiedProgram,
}

impl ProgramViewRef<'_> {
    pub fn program(&self) -> &Program {
        self.inner.program()
    }

    pub fn ir_format_version(&self) -> u32 {
        self.inner.program().format_version
    }

    pub fn to_owned_view(&self) -> ProgramView {
        ProgramView::from_verified(self.inner)
    }
}

pub struct ProgramViewResponseView<'a> {
    pub api_version: u32,
    pub view: ProgramViewRef<'a>,
}

impl ProgramViewResponseView<'_> {
    pub fn into_owned(self) -> ProgramViewResponse {
        ProgramViewResponse {
            api_version: self.api_version,
            view: self.view.to_owned_view(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeConfiguration {
    pub boundary_mode: BoundaryMode,
    pub seed: u64,
    pub custom_execution_limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryEntry {
    pub address: MemoryAddress,
    pub value: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateInstanceRequest {
    pub api_version: u32,
    pub program: ProgramHandle,
    pub input: Vec<u8>,
    pub initial_memory: Vec<MemoryEntry>,
    pub configuration: RuntimeConfiguration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateInstanceResponse {
    pub api_version: u32,
    pub instance: InstanceHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepRequest {
    pub api_version: u32,
    pub instance: InstanceHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StepResponse {
    pub api_version: u32,
    pub result: RuntimeStepResult,
}

/// Full detached step result with an owned post-transition snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStepResult {
    pub attempted_tick: u64,
    pub committed_ticks: u64,
    pub status: VmStatus,
    pub metrics: RuntimeMetricSummary,
    pub events: Vec<VmEvent>,
    pub newly_emitted_output: Vec<u8>,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
    pub snapshot: RuntimeSnapshot,
}

/// Borrowed step response for bounded host serialization.
pub struct StepResponseView<'a> {
    api_version: u32,
    result: VmStepResult,
    snapshot: RuntimeSnapshotView<'a>,
}

impl StepResponseView<'_> {
    pub const fn api_version(&self) -> u32 {
        self.api_version
    }

    pub const fn attempted_tick(&self) -> u64 {
        self.result.attempted_tick
    }

    pub const fn committed_ticks(&self) -> u64 {
        self.result.committed_ticks
    }

    pub const fn status(&self) -> VmStatus {
        self.result.status
    }

    pub const fn metrics(&self) -> &RuntimeMetricSummary {
        &self.result.metrics
    }

    pub fn events(&self) -> &[VmEvent] {
        &self.result.events
    }

    pub fn newly_emitted_output(&self) -> &[u8] {
        &self.result.newly_emitted_output
    }

    pub fn errors(&self) -> &[RuntimeError] {
        &self.result.errors
    }

    pub const fn fault(&self) -> Option<VmFault> {
        self.result.fault
    }

    pub fn snapshot(&self) -> &RuntimeSnapshotView<'_> {
        &self.snapshot
    }

    pub fn into_owned(self) -> StepResponse {
        let Self {
            api_version,
            result,
            snapshot,
        } = self;
        StepResponse {
            api_version,
            result: RuntimeStepResult {
                attempted_tick: result.attempted_tick,
                committed_ticks: result.committed_ticks,
                status: result.status,
                metrics: result.metrics,
                events: result.events,
                newly_emitted_output: result.newly_emitted_output,
                errors: result.errors,
                fault: result.fault,
                snapshot: snapshot.to_owned_snapshot(),
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeThreadSnapshot {
    pub code_grid: CodeGridId,
    pub id: u64,
    pub board: BoardId,
    pub position: Coordinate,
    pub direction: Direction,
    pub register_pointer: u8,
    pub page: Page,
    pub data_stack: Vec<u8>,
    pub instruction_stack: Vec<codegrid_model::InstructionStackItem>,
    pub call_frames: Vec<CallFrameSnapshot>,
    pub phase: ThreadPhaseSnapshot,
    pub random_state: u64,
}

/// Detached Full snapshot of one VM instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSnapshot {
    pub status: VmStatus,
    pub committed_ticks: u64,
    pub registers: [u8; 10],
    pub memory: BTreeMap<MemoryAddress, u8>,
    pub remaining_input: Vec<u8>,
    pub output: Vec<u8>,
    pub runtime_program: CodeGridView,
    pub threads: Vec<RuntimeThreadSnapshot>,
    pub metrics: RuntimeMetrics,
    pub errors: Vec<RuntimeError>,
    pub fault: Option<VmFault>,
}

pub struct RuntimeSnapshotView<'a> {
    inner: VmSnapshotView<'a>,
}

impl RuntimeSnapshotView<'_> {
    pub fn status(&self) -> VmStatus {
        self.inner.status()
    }

    pub fn committed_ticks(&self) -> u64 {
        self.inner.committed_ticks()
    }

    pub fn registers(&self) -> &[u8; 10] {
        self.inner.registers()
    }

    pub fn memory(&self) -> &codegrid_vm::Memory {
        self.inner.memory()
    }

    pub fn remaining_input(&self) -> impl Iterator<Item = &u8> + '_ {
        self.inner.input()
    }

    pub fn output(&self) -> &[u8] {
        self.inner.output()
    }

    pub fn runtime_program(&self) -> &ScopedProgram {
        self.inner.runtime_program()
    }

    pub fn threads(&self) -> impl Iterator<Item = RuntimeThreadSnapshotView<'_>> + '_ {
        self.inner
            .threads()
            .map(|inner| RuntimeThreadSnapshotView { inner })
    }

    pub fn metrics(&self) -> &RuntimeMetrics {
        self.inner.metrics()
    }

    pub fn errors(&self) -> &[RuntimeError] {
        self.inner.errors()
    }

    pub fn fault(&self) -> Option<VmFault> {
        self.inner.fault()
    }

    pub fn to_owned_snapshot(&self) -> RuntimeSnapshot {
        let memory = self
            .memory()
            .allocated_addresses()
            .map(|address| (address.clone(), self.memory().read(address)))
            .collect();
        RuntimeSnapshot {
            status: self.status(),
            committed_ticks: self.committed_ticks(),
            registers: *self.registers(),
            memory,
            remaining_input: self.remaining_input().copied().collect(),
            output: self.output().to_vec(),
            runtime_program: CodeGridView::from_scoped(self.runtime_program()),
            threads: self
                .threads()
                .map(|thread| thread.to_owned_snapshot())
                .collect(),
            metrics: self.metrics().clone(),
            errors: self.errors().to_vec(),
            fault: self.fault(),
        }
    }
}

pub struct RuntimeThreadSnapshotView<'a> {
    inner: ThreadSnapshotView<'a>,
}

impl RuntimeThreadSnapshotView<'_> {
    pub const fn code_grid(&self) -> CodeGridId {
        CodeGridId::Outer
    }

    pub const fn id(&self) -> u64 {
        self.inner.id()
    }

    pub const fn board(&self) -> BoardId {
        self.inner.board()
    }

    pub const fn position(&self) -> Coordinate {
        self.inner.position()
    }

    pub const fn direction(&self) -> Direction {
        self.inner.direction()
    }

    pub const fn register_pointer(&self) -> u8 {
        self.inner.register_pointer()
    }

    pub fn page(&self) -> &Page {
        self.inner.page()
    }

    pub fn data_stack(&self) -> &[u8] {
        self.inner.data_stack()
    }

    pub fn instruction_stack(&self) -> &[codegrid_model::InstructionStackItem] {
        self.inner.instruction_stack()
    }

    pub fn call_frames(&self) -> impl Iterator<Item = CallFrameSnapshot> + '_ {
        self.inner.call_frames()
    }

    pub fn phase(&self) -> ThreadPhaseSnapshot {
        self.inner.phase()
    }

    pub fn random_state(&self) -> u64 {
        self.inner.random_state()
    }

    pub fn to_owned_snapshot(&self) -> RuntimeThreadSnapshot {
        RuntimeThreadSnapshot {
            code_grid: self.code_grid(),
            id: self.id(),
            board: self.board(),
            position: self.position(),
            direction: self.direction(),
            register_pointer: self.register_pointer(),
            page: self.page().clone(),
            data_stack: self.data_stack().to_vec(),
            instruction_stack: self.instruction_stack().to_vec(),
            call_frames: self.call_frames().collect(),
            phase: self.phase(),
            random_state: self.random_state(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RunRequest {
    pub api_version: u32,
    pub instance: InstanceHandle,
    pub max_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum YieldReason {
    /// The host requested a bounded slice and the VM remains runnable.
    TickSliceExhausted,
    /// The host work-unit ceiling interrupted the current Global Tick.
    WorkUnitBudgetExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Halted,
    Error,
    Yielded { reason: YieldReason },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunResponse {
    pub api_version: u32,
    pub status: RunStatus,
    pub events: Vec<VmEvent>,
    pub newly_emitted_output: Vec<u8>,
    pub snapshot: RuntimeSnapshot,
}

pub struct RunResponseView<'a> {
    pub api_version: u32,
    pub status: RunStatus,
    pub events: Vec<VmEvent>,
    pub newly_emitted_output: Vec<u8>,
    pub snapshot: RuntimeSnapshotView<'a>,
}

impl RunResponseView<'_> {
    pub fn into_owned(self) -> RunResponse {
        RunResponse {
            api_version: self.api_version,
            status: self.status,
            events: self.events,
            newly_emitted_output: self.newly_emitted_output,
            snapshot: self.snapshot.to_owned_snapshot(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotRequest {
    pub api_version: u32,
    pub instance: InstanceHandle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotResponse {
    pub api_version: u32,
    pub snapshot: RuntimeSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseProgramRequest {
    pub api_version: u32,
    pub program: ProgramHandle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseResponse {
    pub api_version: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseInstanceRequest {
    pub api_version: u32,
    pub instance: InstanceHandle,
}

/// A rejected host request. These failures are distinct from VM runtime errors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiError {
    UnsupportedVersion {
        received: u32,
        supported: u32,
    },
    SourceLimitExceeded {
        received_bytes: usize,
        maximum: usize,
    },
    ProgramLimitReached {
        maximum: usize,
    },
    InstanceLimitReached {
        maximum: usize,
    },
    InputLimitExceeded {
        received_bytes: usize,
        maximum: usize,
    },
    InitialMemoryLimitExceeded {
        received_entries: usize,
        maximum: usize,
    },
    DuplicateInitialMemoryAddress {
        address: MemoryAddress,
    },
    InvalidConfiguration {
        field: &'static str,
    },
    UnknownProgramHandle {
        handle: ProgramHandle,
    },
    UnknownInstanceHandle {
        handle: InstanceHandle,
    },
    ZeroTickBudget,
    RunTickBudgetExceeded {
        requested: u64,
        maximum: u64,
    },
    InstanceTickBudgetExceeded {
        requested: u64,
        remaining: u64,
        maximum: u64,
    },
    WorkUnitBudgetExceeded {
        maximum: u64,
    },
    HandleSpaceExhausted,
    VmInitialization(VmInitializationError),
}

impl ApiError {
    /// Stable API error category, independent of host-specific wire aliases.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedVersion { .. } => "unsupported_version",
            Self::SourceLimitExceeded { .. } => "source_limit_exceeded",
            Self::ProgramLimitReached { .. } => "program_limit_reached",
            Self::InstanceLimitReached { .. } => "instance_limit_reached",
            Self::InputLimitExceeded { .. } => "input_limit_exceeded",
            Self::InitialMemoryLimitExceeded { .. } => "initial_memory_limit_exceeded",
            Self::DuplicateInitialMemoryAddress { .. } => "duplicate_initial_memory_address",
            Self::InvalidConfiguration { .. } => "invalid_configuration",
            Self::UnknownProgramHandle { .. } => "unknown_program_handle",
            Self::UnknownInstanceHandle { .. } => "unknown_instance_handle",
            Self::ZeroTickBudget => "zero_tick_budget",
            Self::RunTickBudgetExceeded { .. } => "run_tick_budget_exceeded",
            Self::InstanceTickBudgetExceeded { .. } => "instance_tick_budget_exceeded",
            Self::WorkUnitBudgetExceeded { .. } => "work_unit_budget_exceeded",
            Self::HandleSpaceExhausted => "handle_space_exhausted",
            Self::VmInitialization(_) => "vm_initialization_failed",
        }
    }
}

/// A host-neutral registry of compiled programs and isolated VM instances.
///
/// The registry is intentionally not synchronized. A host may put one runtime
/// behind a mutex, actor, worker, or request-local context according to its
/// concurrency model; the VM instances themselves never share execution state.
pub struct RuntimeApi {
    limits: HostLimits,
    programs: BTreeMap<ProgramHandle, VerifiedProgram>,
    instances: BTreeMap<InstanceHandle, Vm>,
    next_program_handle: u64,
    next_instance_handle: u64,
}

impl RuntimeApi {
    pub fn new(limits: HostLimits) -> Self {
        Self {
            limits,
            programs: BTreeMap::new(),
            instances: BTreeMap::new(),
            next_program_handle: 1,
            next_instance_handle: 1,
        }
    }

    pub const fn api_version(&self) -> u32 {
        RUNTIME_API_VERSION
    }

    pub const fn limits(&self) -> HostLimits {
        self.limits
    }

    pub fn check(&self, request: CheckRequest) -> Result<CheckResponse, ApiError> {
        self.validate_version(request.api_version)?;
        self.validate_source_size(request.source.len())?;
        Ok(CheckResponse {
            api_version: RUNTIME_API_VERSION,
            diagnostics: check_source(&request.source),
        })
    }

    pub fn compile(&mut self, request: CompileRequest) -> Result<CompileResponse, ApiError> {
        self.validate_version(request.api_version)?;
        self.validate_source_size(request.source.len())?;

        match compile(&request.source) {
            Ok(program) => {
                if self.programs.len() >= self.limits.max_compiled_programs() {
                    return Err(ApiError::ProgramLimitReached {
                        maximum: self.limits.max_compiled_programs(),
                    });
                }
                self.ensure_program_handle_available()?;
                let handle = self.allocate_program_handle()?;
                self.programs.insert(handle, program);
                Ok(CompileResponse {
                    api_version: RUNTIME_API_VERSION,
                    outcome: CompileOutcome::Compiled { program: handle },
                })
            }
            Err(items) => Ok(CompileResponse {
                api_version: RUNTIME_API_VERSION,
                outcome: CompileOutcome::Diagnostics { items },
            }),
        }
    }

    pub fn program_view(
        &self,
        request: ProgramViewRequest,
    ) -> Result<ProgramViewResponse, ApiError> {
        self.program_view_ref(request)
            .map(ProgramViewResponseView::into_owned)
    }

    pub fn program_view_ref(
        &self,
        request: ProgramViewRequest,
    ) -> Result<ProgramViewResponseView<'_>, ApiError> {
        self.validate_version(request.api_version)?;
        let program =
            self.programs
                .get(&request.program)
                .ok_or(ApiError::UnknownProgramHandle {
                    handle: request.program,
                })?;
        Ok(ProgramViewResponseView {
            api_version: RUNTIME_API_VERSION,
            view: ProgramViewRef { inner: program },
        })
    }

    pub fn create_instance(
        &mut self,
        request: CreateInstanceRequest,
    ) -> Result<CreateInstanceResponse, ApiError> {
        self.validate_version(request.api_version)?;
        if request.input.len() > self.limits.max_input_bytes() {
            return Err(ApiError::InputLimitExceeded {
                received_bytes: request.input.len(),
                maximum: self.limits.max_input_bytes(),
            });
        }
        if request.initial_memory.len() > self.limits.max_initial_memory_entries() {
            return Err(ApiError::InitialMemoryLimitExceeded {
                received_entries: request.initial_memory.len(),
                maximum: self.limits.max_initial_memory_entries(),
            });
        }
        let custom_execution_limit = NonZeroU64::new(request.configuration.custom_execution_limit)
            .ok_or(ApiError::InvalidConfiguration {
                field: "custom_execution_limit",
            })?;
        if self.instances.len() >= self.limits.max_instances() {
            return Err(ApiError::InstanceLimitReached {
                maximum: self.limits.max_instances(),
            });
        }
        self.ensure_instance_handle_available()?;

        let program =
            self.programs
                .get(&request.program)
                .ok_or(ApiError::UnknownProgramHandle {
                    handle: request.program,
                })?;
        let program = program.clone();
        let mut initial_memory = BTreeMap::new();
        for entry in request.initial_memory {
            if initial_memory
                .insert(entry.address.clone(), entry.value)
                .is_some()
            {
                return Err(ApiError::DuplicateInitialMemoryAddress {
                    address: entry.address,
                });
            }
        }
        let configuration = VmConfig::new(
            request.configuration.boundary_mode,
            request.configuration.seed,
            custom_execution_limit,
        );
        let vm = Vm::with_initial_memory(program, request.input, initial_memory, configuration)
            .map_err(ApiError::VmInitialization)?;
        let handle = self.allocate_instance_handle()?;
        self.instances.insert(handle, vm);
        Ok(CreateInstanceResponse {
            api_version: RUNTIME_API_VERSION,
            instance: handle,
        })
    }

    pub fn step(&mut self, request: StepRequest) -> Result<StepResponse, ApiError> {
        self.step_view(request).map(StepResponseView::into_owned)
    }

    /// Applies one VM transition and returns a borrowed Full snapshot for
    /// immediate host serialization.
    ///
    /// Unlike [`RuntimeApi::step`], this method does not materialize an owned
    /// snapshot. The returned view borrows the live VM and prevents further
    /// mutable operations on this runtime until it is dropped.
    pub fn step_view(&mut self, request: StepRequest) -> Result<StepResponseView<'_>, ApiError> {
        self.validate_version(request.api_version)?;
        let maximum = self.limits.max_total_ticks_per_instance();
        let vm =
            self.instances
                .get_mut(&request.instance)
                .ok_or(ApiError::UnknownInstanceHandle {
                    handle: request.instance,
                })?;
        // Terminal VM steps are specified no-ops. They do not commit a tick,
        // so an exhausted cumulative tick budget must not hide their stable
        // terminal result or snapshot.
        let remaining = maximum.saturating_sub(vm.committed_ticks());
        if vm.status() == VmStatus::Running && remaining == 0 {
            return Err(ApiError::InstanceTickBudgetExceeded {
                requested: 1,
                remaining,
                maximum,
            });
        }
        let result = vm
            .step_with_work_limit(self.limits.max_work_units_per_call)
            .map_err(|error| ApiError::WorkUnitBudgetExceeded {
                maximum: error.maximum_work_units,
            })?;
        let snapshot = RuntimeSnapshotView {
            inner: vm.snapshot_view(),
        };
        Ok(StepResponseView {
            api_version: RUNTIME_API_VERSION,
            result,
            snapshot,
        })
    }

    pub fn run(&mut self, request: RunRequest) -> Result<RunResponse, ApiError> {
        self.run_view(request).map(RunResponseView::into_owned)
    }

    /// Executes a bounded run and borrows the post-call snapshot for immediate
    /// host serialization without cloning retained VM state.
    pub fn run_view(&mut self, request: RunRequest) -> Result<RunResponseView<'_>, ApiError> {
        let (status, events, newly_emitted_output) = self.execute_run(request)?;
        let vm = self
            .instances
            .get(&request.instance)
            .ok_or(ApiError::UnknownInstanceHandle {
                handle: request.instance,
            })?;
        Ok(RunResponseView {
            api_version: RUNTIME_API_VERSION,
            status,
            events,
            newly_emitted_output,
            snapshot: RuntimeSnapshotView {
                inner: vm.snapshot_view(),
            },
        })
    }

    fn execute_run(
        &mut self,
        request: RunRequest,
    ) -> Result<(RunStatus, Vec<VmEvent>, Vec<u8>), ApiError> {
        self.validate_version(request.api_version)?;
        if request.max_ticks == 0 {
            return Err(ApiError::ZeroTickBudget);
        }
        if request.max_ticks > self.limits.max_run_ticks_per_call() {
            return Err(ApiError::RunTickBudgetExceeded {
                requested: request.max_ticks,
                maximum: self.limits.max_run_ticks_per_call(),
            });
        }
        let maximum = self.limits.max_total_ticks_per_instance();
        let vm =
            self.instances
                .get_mut(&request.instance)
                .ok_or(ApiError::UnknownInstanceHandle {
                    handle: request.instance,
                })?;
        let remaining = maximum.saturating_sub(vm.committed_ticks());
        if request.max_ticks > remaining {
            return Err(ApiError::InstanceTickBudgetExceeded {
                requested: request.max_ticks,
                remaining,
                maximum,
            });
        }
        let VmRunResult {
            outcome,
            events,
            newly_emitted_output,
            work_limit_exceeded: _,
        } = vm.run_with_work_limit_detailed(request.max_ticks, self.limits.max_work_units_per_call);
        let status = match outcome {
            RunOutcome::Halted => RunStatus::Halted,
            RunOutcome::Error => RunStatus::Error,
            RunOutcome::TickLimitReached => RunStatus::Yielded {
                reason: YieldReason::TickSliceExhausted,
            },
            RunOutcome::WorkLimitReached => RunStatus::Yielded {
                reason: YieldReason::WorkUnitBudgetExhausted,
            },
            RunOutcome::InvalidTickLimit => return Err(ApiError::ZeroTickBudget),
        };
        Ok((
            status,
            events,
            newly_emitted_output
                .into_iter()
                .map(|value| value as u8)
                .collect(),
        ))
    }

    pub fn snapshot(&self, request: SnapshotRequest) -> Result<SnapshotResponse, ApiError> {
        self.validate_version(request.api_version)?;
        let vm = self
            .instances
            .get(&request.instance)
            .ok_or(ApiError::UnknownInstanceHandle {
                handle: request.instance,
            })?;
        Ok(SnapshotResponse {
            api_version: RUNTIME_API_VERSION,
            snapshot: RuntimeSnapshotView {
                inner: vm.snapshot_view(),
            }
            .to_owned_snapshot(),
        })
    }

    /// Borrows a Full snapshot without cloning retained VM state.
    pub fn snapshot_view(
        &self,
        request: SnapshotRequest,
    ) -> Result<RuntimeSnapshotView<'_>, ApiError> {
        self.validate_version(request.api_version)?;
        let vm = self
            .instances
            .get(&request.instance)
            .ok_or(ApiError::UnknownInstanceHandle {
                handle: request.instance,
            })?;
        Ok(RuntimeSnapshotView {
            inner: vm.snapshot_view(),
        })
    }

    /// Alias for [`RuntimeApi::snapshot_view`] used by streaming host projections.
    pub fn snapshot_projection_view(
        &self,
        request: SnapshotRequest,
    ) -> Result<RuntimeSnapshotView<'_>, ApiError> {
        self.snapshot_view(request)
    }

    /// Releases a compiled-program handle. Existing VM instances own verified
    /// program copies and continue running independently.
    pub fn release_program(
        &mut self,
        request: ReleaseProgramRequest,
    ) -> Result<ReleaseResponse, ApiError> {
        self.validate_version(request.api_version)?;
        self.programs
            .remove(&request.program)
            .map(|_| ReleaseResponse {
                api_version: RUNTIME_API_VERSION,
            })
            .ok_or(ApiError::UnknownProgramHandle {
                handle: request.program,
            })
    }

    pub fn release_instance(
        &mut self,
        request: ReleaseInstanceRequest,
    ) -> Result<ReleaseResponse, ApiError> {
        self.validate_version(request.api_version)?;
        self.instances
            .remove(&request.instance)
            .map(|_| ReleaseResponse {
                api_version: RUNTIME_API_VERSION,
            })
            .ok_or(ApiError::UnknownInstanceHandle {
                handle: request.instance,
            })
    }

    fn validate_version(&self, received: u32) -> Result<(), ApiError> {
        if received == RUNTIME_API_VERSION {
            Ok(())
        } else {
            Err(ApiError::UnsupportedVersion {
                received,
                supported: RUNTIME_API_VERSION,
            })
        }
    }

    fn validate_source_size(&self, bytes: usize) -> Result<(), ApiError> {
        if bytes <= self.limits.max_source_bytes() {
            Ok(())
        } else {
            Err(ApiError::SourceLimitExceeded {
                received_bytes: bytes,
                maximum: self.limits.max_source_bytes(),
            })
        }
    }

    fn allocate_program_handle(&mut self) -> Result<ProgramHandle, ApiError> {
        self.ensure_program_handle_available()?;
        let handle = ProgramHandle(self.next_program_handle);
        self.next_program_handle = self
            .next_program_handle
            .checked_add(1)
            .ok_or(ApiError::HandleSpaceExhausted)?;
        Ok(handle)
    }

    fn allocate_instance_handle(&mut self) -> Result<InstanceHandle, ApiError> {
        self.ensure_instance_handle_available()?;
        let handle = InstanceHandle(self.next_instance_handle);
        self.next_instance_handle = self
            .next_instance_handle
            .checked_add(1)
            .ok_or(ApiError::HandleSpaceExhausted)?;
        Ok(handle)
    }

    fn ensure_program_handle_available(&self) -> Result<(), ApiError> {
        self.next_program_handle
            .checked_add(1)
            .map(|_| ())
            .ok_or(ApiError::HandleSpaceExhausted)
    }

    fn ensure_instance_handle_available(&self) -> Result<(), ApiError> {
        self.next_instance_handle
            .checked_add(1)
            .map(|_| ())
            .ok_or(ApiError::HandleSpaceExhausted)
    }
}
