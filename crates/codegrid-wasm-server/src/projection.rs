use codegrid_runtime_api::{
    BoardView, CallFrameSnapshot, CodeGridId, CodeGridView, Coordinate, Diagnostic, Direction,
    ExecutionScope, InstructionKind, MemoryAddress, MemoryLocationId, MemorySpaceId, ProgramView,
    RunStatus, RuntimeError, RuntimeErrorKind, RuntimeMetricSummary, RuntimeMetrics,
    RuntimeSnapshotView, RuntimeThreadSnapshotView, Severity, StaticCellId, StepResponseView,
    ThreadPhaseSnapshot, VmEvent, VmFault, VmStatus, YieldReason,
};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::Serialize;
use std::cell::RefCell;
use std::fmt::Display;

macro_rules! serialize_object {
    ($serializer:expr, { $($key:literal => $value:expr),+ $(,)? }) => {{
        let mut object = $serializer.serialize_map(None)?;
        $(object.serialize_entry($key, &$value)?;)+
        object.end()
    }};
}

pub(crate) struct CheckResponse<'a> {
    pub(crate) api_version: u32,
    pub(crate) diagnostics: &'a [Diagnostic],
}

impl Serialize for CheckResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "check",
            "diagnostics" => DiagnosticSequence(self.diagnostics),
        })
    }
}

pub(crate) struct CompileDiagnosticsResponse<'a> {
    pub(crate) api_version: u32,
    pub(crate) diagnostics: &'a [Diagnostic],
}

impl Serialize for CompileDiagnosticsResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "compile",
            "status" => "diagnostics",
            "diagnostics" => DiagnosticSequence(self.diagnostics),
        })
    }
}

pub(crate) struct CompiledResponse<'a> {
    pub(crate) api_version: u32,
    pub(crate) program: u64,
    pub(crate) view: &'a ProgramView,
}

impl Serialize for CompiledResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "compile",
            "status" => "compiled",
            "program" => DisplayValue(self.program),
            "view" => ProgramViewProjection(self.view),
        })
    }
}

pub(crate) struct ProgramViewResponse<'a> {
    pub(crate) api_version: u32,
    pub(crate) program: u64,
    pub(crate) view: &'a ProgramView,
}

impl Serialize for ProgramViewResponse<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "program_view",
            "program" => DisplayValue(self.program),
            "view" => ProgramViewProjection(self.view),
        })
    }
}

pub(crate) struct StepResponse<'a, 'view> {
    pub(crate) api_version: u32,
    pub(crate) result: &'a StepResponseView<'view>,
}

impl Serialize for StepResponse<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "step",
            "result" => StepResultProjection(self.result),
        })
    }
}

pub(crate) struct SnapshotResponse<'a, 'vm> {
    pub(crate) api_version: u32,
    pub(crate) snapshot: &'a RuntimeSnapshotView<'vm>,
}

impl Serialize for SnapshotResponse<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "snapshot",
            "snapshot" => SnapshotProjection(self.snapshot),
        })
    }
}

pub(crate) struct RunResponse<'a, 'vm> {
    pub(crate) api_version: u32,
    pub(crate) result: &'a codegrid_runtime_api::RunResponseView<'vm>,
}

impl Serialize for RunResponse<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "abi_version" => super::SERVER_ABI_VERSION,
            "api_version" => self.api_version,
            "operation" => "run",
            "status" => RunStatusName(self.result.status),
            "events" => VmEventSequence(&self.result.events),
            "newly_emitted_output" => &self.result.newly_emitted_output,
            "snapshot" => SnapshotProjection(&self.result.snapshot),
        })
    }
}

pub(crate) struct SnapshotProjection<'a, 'vm>(pub(crate) &'a RuntimeSnapshotView<'vm>);

impl Serialize for SnapshotProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let snapshot = self.0;
        let runtime_program = CodeGridView::from_scoped(snapshot.runtime_program());
        serialize_object!(serializer, {
            "status" => VmStatusName(snapshot.status()),
            "committed_ticks" => DisplayValue(snapshot.committed_ticks()),
            "registers" => snapshot.registers(),
            "memory" => MemoryProjection(RefCell::new(snapshot.memory().allocated_addresses().map(|address| {
                (address, snapshot.memory().read(address))
            }))),
            "remaining_input" => ByteSequence(RefCell::new(snapshot.remaining_input())),
            "output" => snapshot.output(),
            "runtime_program" => CodeGridViewProjection(&runtime_program),
            "threads" => SnapshotThreadSequence(RefCell::new(snapshot.threads())),
            "metrics" => RuntimeMetricsProjection(snapshot.metrics()),
            "errors" => RuntimeErrorSequence(snapshot.errors()),
            "fault" => OptionalFaultProjection(snapshot.fault())
        })
    }
}

struct DiagnosticSequence<'a>(&'a [Diagnostic]);

impl Serialize for DiagnosticSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for diagnostic in self.0 {
            sequence.serialize_element(&DiagnosticProjection(diagnostic))?;
        }
        sequence.end()
    }
}

struct DiagnosticProjection<'a>(&'a Diagnostic);

impl Serialize for DiagnosticProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let severity = match self.0.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        serialize_object!(serializer, {
            "code" => self.0.code,
            "error_number" => codegrid_runtime_api::error_number("source", self.0.code).or_else(|| codegrid_runtime_api::error_number("ir", self.0.code)),
            "severity" => severity,
            "message" => &self.0.message,
            "span" => DiagnosticSpan(self.0)
        })
    }
}

struct DiagnosticSpan<'a>(&'a Diagnostic);

impl Serialize for DiagnosticSpan<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "start" => DisplayValue(self.0.span.start),
            "end" => DisplayValue(self.0.span.end)
        })
    }
}

struct ProgramViewProjection<'a>(&'a ProgramView);

impl Serialize for ProgramViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "ir_format_version" => self.0.ir_format_version,
            "outer" => CodeGridViewProjection(&self.0.outer),
            "customs" => CodeGridMapProjection(&self.0.customs)
        })
    }
}

struct CodeGridViewProjection<'a>(&'a CodeGridView);

impl Serialize for CodeGridViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "main" => BoardViewProjection(&self.0.main),
            "functions" => BoardViewMapProjection(&self.0.functions)
        })
    }
}

struct CodeGridMapProjection<'a>(&'a std::collections::BTreeMap<u8, CodeGridView>);

impl Serialize for CodeGridMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, view) in self.0 {
            map.serialize_entry(&id.to_string(), &CodeGridViewProjection(view))?;
        }
        map.end()
    }
}

struct BoardViewMapProjection<'a>(&'a std::collections::BTreeMap<u8, BoardView>);

impl Serialize for BoardViewMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, view) in self.0 {
            map.serialize_entry(&id.to_string(), &BoardViewProjection(view))?;
        }
        map.end()
    }
}

struct BoardViewProjection<'a>(&'a BoardView);

impl Serialize for BoardViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "width" => self.0.width,
            "height" => self.0.height,
            "cells" => CellSequence(&self.0.cells),
            "folded_blocks" => FoldedBlocksProjection(&self.0.folded_blocks)
        })
    }
}

struct CellSequence<'a>(&'a [codegrid_runtime_api::CellView]);

impl Serialize for CellSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for cell in self.0 {
            sequence.serialize_element(&CellProjection(cell))?;
        }
        sequence.end()
    }
}

struct CellProjection<'a>(&'a codegrid_runtime_api::CellView);

impl Serialize for CellProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "prefix" => &self.0.prefix,
            "entry" => &self.0.entry,
            "primary" => &self.0.primary,
            "attachment" => &self.0.attachment
        })
    }
}

struct FoldedBlocksProjection<'a>(&'a std::collections::BTreeMap<u8, Vec<Option<String>>>);

impl Serialize for FoldedBlocksProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, cells) in self.0 {
            map.serialize_entry(&id.to_string(), cells)?;
        }
        map.end()
    }
}

struct StepResultProjection<'a, 'view>(&'a StepResponseView<'view>);

impl Serialize for StepResultProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let result = self.0;
        serialize_object!(serializer, {
            "attempted_tick" => DisplayValue(result.attempted_tick()),
            "committed_ticks" => DisplayValue(result.committed_ticks()),
            "status" => VmStatusName(result.status()),
            "metrics" => RuntimeMetricSummaryProjection(result.metrics()),
            "events" => VmEventSequence(result.events()),
            "newly_emitted_output" => result.newly_emitted_output(),
            "errors" => RuntimeErrorSequence(result.errors()),
            "fault" => OptionalFaultProjection(result.fault()),
            "snapshot" => SnapshotProjection(result.snapshot())
        })
    }
}

struct RuntimeMetricSummaryProjection<'a>(&'a RuntimeMetricSummary);

impl Serialize for RuntimeMetricSummaryProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "global_tick" => DisplayValue(metrics.global_tick()),
            "operation_count" => DisplayValue(metrics.operation_count()),
            "used_cell_count" => DisplayValue(metrics.used_cell_count()),
            "peak_data_stack_usage" => DisplayValue(metrics.peak_data_stack_usage()),
            "peak_instruction_stack_usage" => DisplayValue(metrics.peak_instruction_stack_usage()),
            "peak_call_stack_usage" => DisplayValue(metrics.peak_call_stack_usage()),
            "used_memory_address_count" => DisplayValue(metrics.used_memory_address_count()),
            "instruction_variety" => InstructionVarietySet(metrics.instruction_variety())
        })
    }
}

struct RuntimeMetricsProjection<'a>(&'a RuntimeMetrics);

impl Serialize for RuntimeMetricsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "global_tick" => DisplayValue(metrics.global_tick()),
            "operation_count" => DisplayValue(metrics.operation_count()),
            "used_cell_count" => DisplayValue(metrics.used_cell_count()),
            "used_cells" => UsedCellSequence(metrics.used_cells()),
            "peak_data_stack_usage" => DisplayValue(metrics.peak_data_stack_usage()),
            "peak_instruction_stack_usage" => DisplayValue(metrics.peak_instruction_stack_usage()),
            "peak_call_stack_usage" => DisplayValue(metrics.peak_call_stack_usage()),
            "used_memory_address_count" => DisplayValue(metrics.used_memory_address_count()),
            "used_memory_addresses" => UsedMemorySequence(metrics.used_memory_addresses()),
            "instruction_variety" => InstructionVarietySet(metrics.instruction_variety())
        })
    }
}

struct UsedCellSequence<'a>(&'a std::collections::BTreeSet<StaticCellId>);

impl Serialize for UsedCellSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for cell in self.0 {
            sequence.serialize_element(&StaticCellProjection(cell))?;
        }
        sequence.end()
    }
}

struct StaticCellProjection<'a>(&'a StaticCellId);

impl Serialize for StaticCellProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let cell = self.0;
        match cell.folded_block {
            Some(folded_block) => serialize_object!(serializer, {
                "code_grid" => CodeGridName(cell.code_grid),
                "board" => BoardName(cell.board),
                "folded_block" => folded_block.get(),
                "position" => CoordinateProjection(cell.position)
            }),
            None => serialize_object!(serializer, {
                "code_grid" => CodeGridName(cell.code_grid),
                "board" => BoardName(cell.board),
                "folded_block" => Option::<u8>::None,
                "position" => CoordinateProjection(cell.position)
            }),
        }
    }
}

struct UsedMemorySequence<'a>(&'a std::collections::BTreeSet<MemoryLocationId>);

impl Serialize for UsedMemorySequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for memory in self.0 {
            sequence.serialize_element(&MemoryLocationProjection(memory))?;
        }
        sequence.end()
    }
}

struct MemoryLocationProjection<'a>(&'a MemoryLocationId);

impl Serialize for MemoryLocationProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "space" => MemorySpaceProjection(self.0.space),
            "address" => self.0.address.to_string()
        })
    }
}

struct MemorySpaceProjection(MemorySpaceId);

impl Serialize for MemorySpaceProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            MemorySpaceId::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            MemorySpaceId::CustomInvocation {
                global_tick,
                caller_thread_id,
                custom_id,
            } => serialize_object!(serializer, {
                "kind" => "custom_invocation",
                "global_tick" => DisplayValue(global_tick),
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get()
            }),
        }
    }
}

struct InstructionVarietySet<'a>(&'a std::collections::BTreeSet<InstructionKind>);

impl Serialize for InstructionVarietySet<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for instruction in self.0 {
            sequence.serialize_element(instruction.name())?;
        }
        sequence.end()
    }
}

struct MemoryProjection<I>(RefCell<I>);

impl<'a, I> Serialize for MemoryProjection<I>
where
    I: Iterator<Item = (&'a MemoryAddress, u8)>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for (address, value) in self.0.borrow_mut().by_ref() {
            sequence.serialize_element(&MemoryEntryProjection {
                address: address.to_string(),
                value,
            })?;
        }
        sequence.end()
    }
}

struct MemoryEntryProjection {
    address: String,
    value: u8,
}

impl Serialize for MemoryEntryProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "address" => &self.address,
            "value" => self.value
        })
    }
}

struct ByteSequence<I>(RefCell<I>);

impl<I> Serialize for ByteSequence<I>
where
    I: Iterator,
    I::Item: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in self.0.borrow_mut().by_ref() {
            sequence.serialize_element(&value)?;
        }
        sequence.end()
    }
}

struct SnapshotThreadSequence<I>(RefCell<I>);

impl<'a, I> Serialize for SnapshotThreadSequence<I>
where
    I: Iterator<Item = RuntimeThreadSnapshotView<'a>>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for thread in self.0.borrow_mut().by_ref() {
            sequence.serialize_element(&SnapshotThreadProjection(&thread))?;
        }
        sequence.end()
    }
}

struct SnapshotThreadProjection<'a, 'vm>(&'a RuntimeThreadSnapshotView<'vm>);

impl Serialize for SnapshotThreadProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let thread = self.0;
        serialize_thread(
            serializer,
            CodeGridId::Outer,
            thread.id(),
            thread.board(),
            thread.position(),
            thread.direction(),
            thread.register_pointer(),
            thread.status_flag(),
            thread.private_registers().copied(),
            thread.page().to_string(),
            thread.data_stack(),
            thread.instruction_stack().iter().map(|item| item.code()),
            thread.call_frames(),
            thread.phase(),
            thread.random_state(),
        )
    }
}

fn serialize_thread<S, I, J>(
    serializer: S,
    code_grid: CodeGridId,
    id: u64,
    board: codegrid_runtime_api::BoardId,
    position: Coordinate,
    direction: Direction,
    register_pointer: u8,
    status_flag: u8,
    private_registers: Option<[u8; 10]>,
    page: String,
    data_stack: &[u8],
    instruction_stack: I,
    call_frames: J,
    phase: ThreadPhaseSnapshot,
    random_state: u64,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    I: Iterator<Item = u8>,
    J: Iterator<Item = CallFrameSnapshot>,
{
    let mut object = serializer.serialize_map(None)?;
    object.serialize_entry("code_grid", &CodeGridName(code_grid))?;
    object.serialize_entry("id", &DisplayValue(id))?;
    object.serialize_entry("board", &BoardName(board))?;
    object.serialize_entry("position", &CoordinateProjection(position))?;
    object.serialize_entry("direction", &DirectionName(direction))?;
    object.serialize_entry("register_pointer", &register_pointer)?;
    object.serialize_entry("status_flag", &status_flag)?;
    object.serialize_entry("private_registers", &private_registers)?;
    object.serialize_entry("page", &page)?;
    object.serialize_entry("data_stack", &data_stack)?;
    object.serialize_entry(
        "instruction_stack",
        &InstructionStackCodes(RefCell::new(instruction_stack)),
    )?;
    object.serialize_entry("call_frames", &CallFrameSequence(RefCell::new(call_frames)))?;
    object.serialize_entry("phase", &ThreadPhaseProjection(phase))?;
    object.serialize_entry("random_state", &DisplayValue(random_state))?;
    object.end()
}

struct InstructionStackCodes<I>(RefCell<I>);

impl<I> Serialize for InstructionStackCodes<I>
where
    I: Iterator<Item = u8>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for code in self.0.borrow_mut().by_ref() {
            sequence.serialize_element(&code)?;
        }
        sequence.end()
    }
}

struct CallFrameSequence<I>(RefCell<I>);

impl<I> Serialize for CallFrameSequence<I>
where
    I: Iterator<Item = CallFrameSnapshot>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for frame in self.0.borrow_mut().by_ref() {
            sequence.serialize_element(&CallFrameProjection(frame))?;
        }
        sequence.end()
    }
}

struct CallFrameProjection(CallFrameSnapshot);

impl Serialize for CallFrameProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "caller_board" => BoardName(self.0.caller_board),
            "call_position" => CoordinateProjection(self.0.call_position),
            "saved_direction" => DirectionName(self.0.saved_direction),
            "saved_registers" => self.0.saved_registers,
            "saved_register_pointer" => self.0.saved_register_pointer,
            "saved_status_flag" => self.0.saved_status_flag
        })
    }
}

struct ThreadPhaseProjection(ThreadPhaseSnapshot);

impl Serialize for ThreadPhaseProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use ThreadPhaseSnapshot as Phase;
        match self.0 {
            Phase::Normal => serialize_object!(serializer, { "kind" => "normal" }),
            Phase::AfterCall => serialize_object!(serializer, { "kind" => "after_call" }),
            Phase::Repeat { total, completed } => serialize_object!(serializer, {
                "kind" => "repeat", "total" => total, "completed" => completed
            }),
            Phase::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold",
                "fold_id" => fold_id.get(),
                "internal_position" => CoordinateProjection(internal_position),
                "internal_direction" => DirectionName(internal_direction),
                "saved_outer_direction" => DirectionName(saved_outer_direction)
            }),
            Phase::FoldResume {
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold_resume",
                "saved_outer_direction" => DirectionName(saved_outer_direction)
            }),
            Phase::Terminated => serialize_object!(serializer, { "kind" => "terminated" }),
        }
    }
}

struct VmEventSequence<'a>(&'a [VmEvent]);

impl Serialize for VmEventSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for event in self.0 {
            sequence.serialize_element(&VmEventProjection(event))?;
        }
        sequence.end()
    }
}

struct VmEventProjection<'a>(&'a VmEvent);

impl Serialize for VmEventProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            VmEvent::CellReached {
                scope,
                thread_id,
                cell,
            } => serialize_object!(serializer, {
                "kind" => "cell_reached",
                "scope" => ExecutionScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "cell" => StaticCellProjection(cell)
            }),
            VmEvent::InputConsumed {
                scope,
                thread_id,
                value,
            } => serialize_object!(serializer, {
                "kind" => "input_consumed",
                "scope" => ExecutionScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "value" => *value
            }),
            VmEvent::RegisterChanged {
                scope,
                register,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "register_changed",
                "scope" => ExecutionScopeProjection(*scope),
                "register" => register,
                "old" => old,
                "new" => new
            }),
            VmEvent::MemoryChanged {
                scope,
                location,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "memory_changed",
                "scope" => ExecutionScopeProjection(*scope),
                "location" => MemoryLocationProjection(location),
                "old" => old,
                "new" => new
            }),
            VmEvent::CodeChanged {
                scope,
                cell,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "code_changed",
                "scope" => ExecutionScopeProjection(*scope),
                "cell" => StaticCellProjection(cell),
                "old" => old.map(|instruction| instruction.token().to_string()),
                "new" => new.map(|instruction| instruction.token().to_string())
            }),
            VmEvent::ThreadChanged {
                scope,
                before,
                after,
            } => {
                let mut object = serializer.serialize_map(None)?;
                object.serialize_entry("kind", "thread_changed")?;
                object.serialize_entry("scope", &ExecutionScopeProjection(*scope))?;
                object.serialize_entry(
                    "before",
                    &EventThreadProjection {
                        code_grid: code_grid_for_scope(*scope),
                        id: before.id,
                        board: before.board,
                        position: before.position,
                        direction: before.direction,
                        register_pointer: before.register_pointer,
                        status_flag: before.status_flag,
                        private_registers: before.private_registers,
                        page: before.page.to_string(),
                        data_stack: before.data_stack.as_slice(),
                        instruction_codes: before.instruction_stack.iter().map(|item| item.code()),
                        call_frames: before.call_stack.iter().copied(),
                        phase: before.phase,
                        random_state: before.random_state,
                    },
                )?;
                object.serialize_entry(
                    "after",
                    &EventThreadProjection {
                        code_grid: code_grid_for_scope(*scope),
                        id: after.id,
                        board: after.board,
                        position: after.position,
                        direction: after.direction,
                        register_pointer: after.register_pointer,
                        status_flag: after.status_flag,
                        private_registers: after.private_registers,
                        page: after.page.to_string(),
                        data_stack: after.data_stack.as_slice(),
                        instruction_codes: after.instruction_stack.iter().map(|item| item.code()),
                        call_frames: after.call_stack.iter().copied(),
                        phase: after.phase,
                        random_state: after.random_state,
                    },
                )?;
                object.end()
            }
        }
    }
}

struct EventThreadProjection<'a, I, J> {
    code_grid: CodeGridId,
    id: u64,
    board: codegrid_runtime_api::BoardId,
    position: Coordinate,
    direction: Direction,
    register_pointer: u8,
    status_flag: u8,
    private_registers: Option<[u8; 10]>,
    page: String,
    data_stack: &'a [u8],
    instruction_codes: I,
    call_frames: J,
    phase: ThreadPhaseSnapshot,
    random_state: u64,
}

impl<I, J> Serialize for EventThreadProjection<'_, I, J>
where
    I: Iterator<Item = u8> + Clone,
    J: Iterator<Item = CallFrameSnapshot> + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_thread(
            serializer,
            self.code_grid,
            self.id,
            self.board,
            self.position,
            self.direction,
            self.register_pointer,
            self.status_flag,
            self.private_registers,
            self.page.clone(),
            self.data_stack,
            self.instruction_codes.clone(),
            self.call_frames.clone(),
            self.phase,
            self.random_state,
        )
    }
}

fn code_grid_for_scope(scope: ExecutionScope) -> CodeGridId {
    match scope {
        ExecutionScope::Outer => CodeGridId::Outer,
        ExecutionScope::Custom { custom_id, .. } => CodeGridId::Custom(custom_id),
    }
}

struct ExecutionScopeProjection(ExecutionScope);

impl Serialize for ExecutionScopeProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ExecutionScope::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            ExecutionScope::Custom {
                caller_thread_id,
                custom_id,
                internal_tick,
            } => serialize_object!(serializer, {
                "kind" => "custom",
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get(),
                "internal_tick" => DisplayValue(internal_tick)
            }),
        }
    }
}

struct RuntimeErrorSequence<'a>(&'a [RuntimeError]);

impl Serialize for RuntimeErrorSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for error in self.0 {
            sequence.serialize_element(&RuntimeErrorProjection(error))?;
        }
        sequence.end()
    }
}

struct RuntimeErrorProjection<'a>(&'a RuntimeError);

impl Serialize for RuntimeErrorProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code" => self.0.code(),
            "error_number" => codegrid_runtime_api::error_number("vm", self.0.code()),
            "global_tick" => DisplayValue(self.0.global_tick()),
            "scope" => ExecutionScopeProjection(self.0.scope()),
            "details" => RuntimeErrorDetailsProjection(self.0.kind())
        })
    }
}

struct RuntimeErrorDetailsProjection<'a>(&'a RuntimeErrorKind);

impl Serialize for RuntimeErrorDetailsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use RuntimeErrorKind as Kind;
        match self.0 {
            Kind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            }
            | Kind::ConcurrentCallerStackReadWriteConflict {
                internal_thread_ids,
            }
            | Kind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids,
            } => {
                serialize_object!(serializer, { "internal_thread_ids" => ThreadIdSequence(internal_thread_ids) })
            }
            Kind::ConcurrentCodeWriteConflict { cell, thread_ids } => {
                serialize_object!(serializer, {
                    "cell" => StaticCellProjection(cell),
                    "thread_ids" => ThreadIdSequence(thread_ids)
                })
            }
            Kind::ConcurrentInputConflict { thread_ids }
            | Kind::ConcurrentOutputConflict { thread_ids } => serialize_object!(serializer, {
                "thread_ids" => ThreadIdSequence(thread_ids)
            }),
            Kind::ConcurrentMemoryWriteConflict {
                address,
                thread_ids,
            } => serialize_object!(serializer, {
                "address" => address.to_string(),
                "thread_ids" => ThreadIdSequence(thread_ids)
            }),
            Kind::ConcurrentWriteConflict {
                register,
                thread_ids,
            } => serialize_object!(serializer, {
                "register" => register,
                "thread_ids" => ThreadIdSequence(thread_ids)
            }),
            Kind::CustomExecutionLimitExceeded { limit } => serialize_object!(serializer, {
                "limit" => DisplayValue(limit)
            }),
            Kind::OutOfBounds {
                thread_id,
                board,
                position,
                direction,
            } => serialize_object!(serializer, {
                "thread_id" => DisplayValue(thread_id),
                "board" => BoardName(*board),
                "position" => CoordinateProjection(*position),
                "direction" => DirectionName(*direction)
            }),
            Kind::ReturnWithoutCall {
                thread_id,
                board,
                position,
            } => serialize_object!(serializer, {
                "thread_id" => DisplayValue(thread_id),
                "board" => BoardName(*board),
                "position" => CoordinateProjection(*position)
            }),
        }
    }
}

struct ThreadIdSequence<'a>(&'a [u64]);

impl Serialize for ThreadIdSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for id in self.0 {
            sequence.serialize_element(&DisplayValue(*id))?;
        }
        sequence.end()
    }
}

struct OptionalFaultProjection(Option<VmFault>);

impl Serialize for OptionalFaultProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            Some(fault) => serializer.serialize_some(&FaultProjection(fault)),
            None => serializer.serialize_none(),
        }
    }
}

struct FaultProjection(VmFault);

impl Serialize for FaultProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            VmFault::MetricCounterOverflow(counter) => serialize_object!(serializer, {
                "kind" => "metric_counter_overflow", "error_number" => codegrid_runtime_api::error_number("fault", "metric_counter_overflow"),
                "counter" => super::metric_counter_name(counter)
            }),
            VmFault::GlobalTickOverflow => {
                serialize_object!(serializer, { "kind" => "global_tick_overflow", "error_number" => codegrid_runtime_api::error_number("fault", "global_tick_overflow") })
            }
            VmFault::InternalInvariantViolation => {
                serialize_object!(serializer, { "kind" => "internal_invariant_violation", "error_number" => codegrid_runtime_api::error_number("fault", "internal_invariant_violation") })
            }
        }
    }
}

struct VmStatusName(VmStatus);

impl Serialize for VmStatusName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let name = match self.0 {
            VmStatus::Running => "running",
            VmStatus::Halted => "halted",
            VmStatus::Error => "error",
        };
        serializer.serialize_str(name)
    }
}

struct RunStatusName(RunStatus);

impl Serialize for RunStatusName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            RunStatus::Halted => serializer.serialize_str("halted"),
            RunStatus::Error => serializer.serialize_str("error"),
            RunStatus::Yielded {
                reason: YieldReason::TickSliceExhausted,
            } => serializer.serialize_str("tick_limit_reached"),
            RunStatus::Yielded {
                reason: YieldReason::WorkUnitBudgetExhausted,
            } => serializer.serialize_str("work_limit_reached"),
        }
    }
}

struct CoordinateProjection(Coordinate);

impl Serialize for CoordinateProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, { "x" => self.0.x, "y" => self.0.y })
    }
}

struct DirectionName(Direction);

impl Serialize for DirectionName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(super::direction_name(self.0))
    }
}

struct BoardName(codegrid_runtime_api::BoardId);

impl Serialize for BoardName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            codegrid_runtime_api::BoardId::Main => serializer.serialize_str("main"),
            codegrid_runtime_api::BoardId::Function(slot) => {
                serializer.collect_str(&format_args!("function:{}", slot.get()))
            }
        }
    }
}

struct CodeGridName(CodeGridId);

impl Serialize for CodeGridName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            CodeGridId::Outer => serializer.serialize_str("outer"),
            CodeGridId::Custom(slot) => {
                serializer.collect_str(&format_args!("custom:{}", slot.get()))
            }
        }
    }
}

struct DisplayValue<T>(T);

impl<T: Display> Serialize for DisplayValue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}
