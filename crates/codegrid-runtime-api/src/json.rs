//! Shared exact JSON projections for Full VM host responses.
//! Serializers consume read-only views and never execute language semantics.
use crate::*;
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::Serialize;
use std::fmt::Display;

macro_rules! serialize_object {
    ($serializer:expr, { $($key:literal => $value:expr),+ $(,)? }) => {{
        let mut object = $serializer.serialize_map(None)?;
        $(object.serialize_entry($key, &$value)?;)+
        object.end()
    }};
}

pub struct CheckResponseProjection<'a> {
    pub api_version: u32,
    pub diagnostics: &'a [Diagnostic],
}

impl Serialize for CheckResponseProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "diagnostics" => DiagnosticSequence(self.diagnostics),
        })
    }
}

pub struct CompileDiagnosticsProjection<'a> {
    pub api_version: u32,
    pub diagnostics: &'a [Diagnostic],
}

impl Serialize for CompileDiagnosticsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "outcome" => DiagnosticOutcomeProjection(self.diagnostics),
        })
    }
}

pub struct DiagnosticOutcomeProjection<'a>(pub &'a [Diagnostic]);

impl Serialize for DiagnosticOutcomeProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "kind" => "diagnostics",
            "items" => DiagnosticSequence(self.0),
        })
    }
}

pub struct DiagnosticSequence<'a>(pub &'a [Diagnostic]);

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

pub struct DiagnosticProjection<'a>(pub &'a Diagnostic);

impl Serialize for DiagnosticProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code" => self.0.code,
            "error_number" => crate::error_number("source", self.0.code).or_else(|| crate::error_number("ir", self.0.code)),
            "severity" => match self.0.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            "message" => &self.0.message,
            "span" => DiagnosticSpanProjection {
                start: self.0.span.start,
                end: self.0.span.end,
            },
        })
    }
}

pub struct DiagnosticSpanProjection {
    pub start: usize,
    pub end: usize,
}

impl Serialize for DiagnosticSpanProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "start" => DisplayValue(self.start),
            "end" => DisplayValue(self.end),
        })
    }
}

pub struct DisplayValue<T>(pub T);

impl<T: Display> Serialize for DisplayValue<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

pub struct CompileProjection {
    pub api_version: u32,
    pub program: u64,
}

impl Serialize for CompileProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(2))?;
        object.serialize_entry("api_version", &self.api_version)?;
        object.serialize_entry("outcome", &CompiledOutcomeProjection(self.program))?;
        object.end()
    }
}

pub struct CompiledOutcomeProjection(pub u64);

impl Serialize for CompiledOutcomeProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "kind" => "compiled",
            "program" => DisplayValue(self.0),
        })
    }
}

pub struct ProgramViewResponseProjection<'a> {
    pub api_version: u32,
    pub view: &'a ProgramView,
}

impl Serialize for ProgramViewResponseProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "view" => ProgramViewProjection(self.view),
        })
    }
}

pub struct ProgramViewProjection<'a>(pub &'a ProgramView);

impl Serialize for ProgramViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(3))?;
        object.serialize_entry("ir_format_version", &self.0.ir_format_version)?;
        object.serialize_entry("outer", &CodeGridViewProjection(&self.0.outer))?;
        object.serialize_entry("customs", &CodeGridMapProjection(&self.0.customs))?;
        object.end()
    }
}

pub struct CodeGridViewProjection<'a>(pub &'a CodeGridView);

impl Serialize for CodeGridViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "main" => BoardViewProjection(&self.0.main),
            "functions" => BoardViewMapProjection(&self.0.functions),
        })
    }
}

pub struct CodeGridMapProjection<'a>(pub &'a std::collections::BTreeMap<u8, CodeGridView>);

impl Serialize for CodeGridMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, grid) in self.0 {
            object.serialize_entry(&id.to_string(), &CodeGridViewProjection(grid))?;
        }
        object.end()
    }
}

pub struct BoardViewMapProjection<'a>(pub &'a std::collections::BTreeMap<u8, BoardView>);

impl Serialize for BoardViewMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, board) in self.0 {
            object.serialize_entry(&id.to_string(), &BoardViewProjection(board))?;
        }
        object.end()
    }
}

pub struct BoardViewProjection<'a>(pub &'a BoardView);

impl Serialize for BoardViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "width" => self.0.width,
            "height" => self.0.height,
            "cells" => CellViewSequence(&self.0.cells),
            "folded_blocks" => FoldedViewMapProjection(&self.0.folded_blocks),
        })
    }
}

pub struct CellViewSequence<'a>(pub &'a [crate::CellView]);

impl Serialize for CellViewSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for cell in self.0 {
            sequence.serialize_element(&CellViewProjection(cell))?;
        }
        sequence.end()
    }
}

pub struct CellViewProjection<'a>(pub &'a crate::CellView);

impl Serialize for CellViewProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "prefix" => &self.0.prefix,
            "entry" => &self.0.entry,
            "primary" => &self.0.primary,
            "attachment" => &self.0.attachment,
        })
    }
}

pub struct FoldedViewMapProjection<'a>(pub &'a std::collections::BTreeMap<u8, Vec<Option<String>>>);

impl Serialize for FoldedViewMapProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(self.0.len()))?;
        for (id, cells) in self.0 {
            object.serialize_entry(&id.to_string(), cells)?;
        }
        object.end()
    }
}

pub struct SnapshotProjection<'snapshot, 'runtime> {
    pub snapshot: &'snapshot RuntimeSnapshotView<'runtime>,
}

impl<'snapshot, 'runtime> SnapshotProjection<'snapshot, 'runtime> {
    pub fn new(snapshot: &'snapshot RuntimeSnapshotView<'runtime>) -> Self {
        Self { snapshot }
    }
}

impl Serialize for SnapshotProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let snapshot = self.snapshot;
        let runtime_program = CodeGridView::from_scoped(snapshot.runtime_program());
        let mut object = serializer.serialize_map(Some(11))?;
        object.serialize_entry("status", vm_status_name(snapshot.status()))?;
        object.serialize_entry("committed_steps", &DisplayValue(snapshot.committed_steps()))?;
        object.serialize_entry("registers", snapshot.registers())?;
        object.serialize_entry("memory", &SnapshotMemorySequence(snapshot))?;
        object.serialize_entry("remaining_input", &InputSequence(snapshot))?;
        object.serialize_entry("output", snapshot.output())?;
        object.serialize_entry("runtime_program", &CodeGridViewProjection(&runtime_program))?;
        object.serialize_entry("threads", &SnapshotThreadSequence(snapshot))?;
        object.serialize_entry("metrics", &RuntimeMetricsProjection(snapshot.metrics()))?;
        object.serialize_entry("errors", &RuntimeErrorSequence(snapshot.errors()))?;
        object.serialize_entry("fault", &OptionalFaultProjection(snapshot.fault()))?;
        object.end()
    }
}

pub struct SnapshotMemorySequence<'a, 'runtime>(pub &'a RuntimeSnapshotView<'runtime>);

impl Serialize for SnapshotMemorySequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let memory = self.0.memory();
        let mut sequence = serializer.serialize_seq(None)?;
        for address in memory.allocated_addresses() {
            sequence.serialize_element(&MemoryEntryProjection {
                address,
                value: memory.read(address),
            })?;
        }
        sequence.end()
    }
}

pub struct MemoryEntryProjection<'a> {
    pub address: &'a MemoryAddress,
    pub value: u8,
}

impl Serialize for MemoryEntryProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "address" => DisplayValue(self.address),
            "value" => self.value,
        })
    }
}

pub struct SnapshotThreadSequence<'a, 'runtime>(pub &'a RuntimeSnapshotView<'runtime>);

impl Serialize for SnapshotThreadSequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for thread in self.0.threads() {
            sequence.serialize_element(&RuntimeThreadProjection(thread))?;
        }
        sequence.end()
    }
}

pub struct InstructionCodes<I>(pub I);

impl<I> Serialize for InstructionCodes<I>
where
    I: Iterator<Item = u8> + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for code in self.0.clone() {
            sequence.serialize_element(&code)?;
        }
        sequence.end()
    }
}

pub struct CallFrameSequence<'a>(pub &'a [CallFrameSnapshot]);

impl Serialize for CallFrameSequence<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for frame in self.0 {
            sequence.serialize_element(&CallFrameProjection(*frame))?;
        }
        sequence.end()
    }
}

pub struct ThreadProjection<'a, I> {
    pub code_grid: CodeGridId,
    pub id: u64,
    pub board: BoardId,
    pub position: Coordinate,
    pub direction: Direction,
    pub register_pointer: u8,
    pub status_flag: u8,
    pub private_registers: Option<[u8; 10]>,
    pub page: &'a crate::Page,
    pub data_stack: &'a [u8],
    pub instruction_stack: InstructionCodes<I>,
    pub call_frames: Vec<CallFrameSnapshot>,
    pub phase: ThreadPhaseSnapshot,
    pub random_state: u64,
}

impl<'a, I> ThreadProjection<'a, I>
where
    I: Iterator<Item = u8> + Clone,
{
    pub fn from_fields(
        code_grid: CodeGridId,
        id: u64,
        board: BoardId,
        position: Coordinate,
        direction: Direction,
        register_pointer: u8,
        status_flag: u8,
        private_registers: Option<[u8; 10]>,
        page: &'a crate::Page,
        data_stack: &'a [u8],
        instruction_stack: I,
        call_frames: Vec<CallFrameSnapshot>,
        phase: ThreadPhaseSnapshot,
        random_state: u64,
    ) -> Self {
        Self {
            code_grid,
            id,
            board,
            position,
            direction,
            register_pointer,
            status_flag,
            private_registers,
            page,
            data_stack,
            instruction_stack: InstructionCodes(instruction_stack),
            call_frames,
            phase,
            random_state,
        }
    }
}

pub struct RuntimeThreadProjection<'a>(pub RuntimeThreadSnapshotView<'a>);

impl Serialize for RuntimeThreadProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let thread = &self.0;
        let call_frames = thread.call_frames().collect::<Vec<_>>();
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(thread.code_grid()),
            "id" => DisplayValue(thread.id()),
            "board" => BoardIdProjection(thread.board()),
            "position" => CoordinateProjection(thread.position()),
            "direction" => direction_name(thread.direction()),
            "register_pointer" => thread.register_pointer(),
            "status_flag" => thread.status_flag(),
            "private_registers" => thread.private_registers(),
            "page" => DisplayValue(thread.page()),
            "data_stack" => thread.data_stack(),
            "instruction_stack" => InstructionCodes(thread.instruction_stack().iter().map(|item| item.code())),
            "call_frames" => CallFrameSequence(&call_frames),
            "phase" => ThreadPhaseProjection(thread.phase()),
            "random_state" => DisplayValue(thread.random_state()),
        })
    }
}

impl<I> Serialize for ThreadProjection<'_, I>
where
    I: Iterator<Item = u8> + Clone,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(self.code_grid),
            "id" => DisplayValue(self.id),
            "board" => BoardIdProjection(self.board),
            "position" => CoordinateProjection(self.position),
            "direction" => direction_name(self.direction),
            "register_pointer" => self.register_pointer,
            "status_flag" => self.status_flag,
            "private_registers" => self.private_registers,
            "page" => DisplayValue(self.page),
            "data_stack" => self.data_stack,
            "instruction_stack" => &self.instruction_stack,
            "call_frames" => CallFrameSequence(&self.call_frames),
            "phase" => ThreadPhaseProjection(self.phase),
            "random_state" => DisplayValue(self.random_state),
        })
    }
}

pub struct CallFrameProjection(pub CallFrameSnapshot);

impl Serialize for CallFrameProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "caller_board" => BoardIdProjection(self.0.caller_board),
            "call_position" => CoordinateProjection(self.0.call_position),
            "saved_direction" => direction_name(self.0.saved_direction),
            "saved_registers" => self.0.saved_registers,
            "saved_register_pointer" => self.0.saved_register_pointer,
            "saved_status_flag" => self.0.saved_status_flag,
        })
    }
}

pub struct ThreadPhaseProjection(pub ThreadPhaseSnapshot);

impl Serialize for ThreadPhaseProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ThreadPhaseSnapshot::Normal => serialize_object!(serializer, { "kind" => "normal" }),
            ThreadPhaseSnapshot::AfterCall => {
                serialize_object!(serializer, { "kind" => "after_call" })
            }
            ThreadPhaseSnapshot::Repeat { total, completed } => serialize_object!(serializer, {
                "kind" => "repeat",
                "total" => total,
                "completed" => completed,
            }),
            ThreadPhaseSnapshot::Fold {
                fold_id,
                internal_position,
                internal_direction,
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold",
                "fold_id" => fold_id.get(),
                "internal_position" => CoordinateProjection(internal_position),
                "internal_direction" => direction_name(internal_direction),
                "saved_outer_direction" => direction_name(saved_outer_direction),
            }),
            ThreadPhaseSnapshot::FoldResume {
                saved_outer_direction,
            } => serialize_object!(serializer, {
                "kind" => "fold_resume",
                "saved_outer_direction" => direction_name(saved_outer_direction),
            }),
            ThreadPhaseSnapshot::Terminated => {
                serialize_object!(serializer, { "kind" => "terminated" })
            }
        }
    }
}
pub struct SnapshotResponseProjection<'snapshot, 'runtime> {
    pub api_version: u32,
    pub snapshot: SnapshotProjection<'snapshot, 'runtime>,
}

impl Serialize for SnapshotResponseProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.api_version,
            "snapshot" => &self.snapshot,
        })
    }
}

pub struct RunProjection<'snapshot, 'runtime> {
    pub api_version: u32,
    pub status: RunStatus,
    pub events: &'snapshot [VmEvent],
    pub newly_emitted_output: &'snapshot [u8],
    pub snapshot: SnapshotProjection<'snapshot, 'runtime>,
}

impl Serialize for RunProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut object = serializer.serialize_map(Some(5))?;
        object.serialize_entry("api_version", &self.api_version)?;
        object.serialize_entry("status", &RunStatusProjection(self.status))?;
        object.serialize_entry("events", &VmEventSequence(self.events))?;
        object.serialize_entry("newly_emitted_output", self.newly_emitted_output)?;
        object.serialize_entry("snapshot", &self.snapshot)?;
        object.end()
    }
}

pub struct RunStatusProjection(pub RunStatus);

impl Serialize for RunStatusProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            RunStatus::Halted => serialize_object!(serializer, { "kind" => "halted" }),
            RunStatus::Error => serialize_object!(serializer, { "kind" => "error" }),
            RunStatus::Yielded { reason } => {
                let reason = match reason {
                    YieldReason::StepSliceExhausted => "step_slice_exhausted",
                    YieldReason::WorkUnitBudgetExhausted => "work_unit_budget_exhausted",
                };
                serialize_object!(serializer, {
                    "kind" => "yielded",
                    "reason" => reason,
                })
            }
        }
    }
}

pub struct StepResponseProjection<'a, 'view> {
    pub response: &'a StepResponseView<'view>,
}

impl Serialize for StepResponseProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "api_version" => self.response.api_version(),
            "result" => StepResultProjection(self.response),
        })
    }
}

pub struct StepResultProjection<'a, 'view>(pub &'a StepResponseView<'view>);

impl Serialize for StepResultProjection<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let result = self.0;
        serialize_object!(serializer, {
            "attempted_transition_id" => DisplayValue(result.attempted_transition_id()),
            "committed_steps" => DisplayValue(result.committed_steps()),
            "status" => vm_status_name(result.status()),
            "metrics" => RuntimeMetricSummaryProjection(result.metrics()),
            "events" => VmEventSequence(result.events()),
            "newly_emitted_output" => result.newly_emitted_output(),
            "errors" => RuntimeErrorSequence(result.errors()),
            "fault" => OptionalFaultProjection(result.fault()),
            "snapshot" => SnapshotProjection::new(result.snapshot()),
        })
    }
}

pub struct InputSequence<'snapshot, 'runtime>(pub &'snapshot RuntimeSnapshotView<'runtime>);

impl Serialize for InputSequence<'_, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(None)?;
        for value in self.0.remaining_input() {
            sequence.serialize_element(value)?;
        }
        sequence.end()
    }
}

pub struct CoordinateProjection(pub Coordinate);

impl Serialize for CoordinateProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "x" => self.0.x,
            "y" => self.0.y,
        })
    }
}

pub struct CodeGridIdProjection(pub CodeGridId);

impl Serialize for CodeGridIdProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            CodeGridId::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            CodeGridId::Custom(id) => serialize_object!(serializer, {
                "kind" => "custom",
                "id" => id.get(),
            }),
        }
    }
}

pub struct BoardIdProjection(pub BoardId);

impl Serialize for BoardIdProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            BoardId::Main => serialize_object!(serializer, { "kind" => "main" }),
            BoardId::Function(id) => serialize_object!(serializer, {
                "kind" => "function",
                "id" => id.get(),
            }),
        }
    }
}

pub struct ScopeProjection(pub ExecutionScope);

impl Serialize for ScopeProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            ExecutionScope::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            ExecutionScope::Custom {
                caller_thread_id,
                custom_id,
                internal_step,
            } => serialize_object!(serializer, {
                "kind" => "custom",
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get(),
                "internal_step" => DisplayValue(internal_step),
            }),
        }
    }
}

pub struct StaticCellProjection(pub StaticCellId);

impl Serialize for StaticCellProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code_grid" => CodeGridIdProjection(self.0.code_grid),
            "board" => BoardIdProjection(self.0.board),
            "folded_block" => self.0.folded_block.map(|id| id.get()),
            "position" => CoordinateProjection(self.0.position),
        })
    }
}

pub struct MemorySpaceProjection(pub MemorySpaceId);

impl Serialize for MemorySpaceProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            MemorySpaceId::Outer => serialize_object!(serializer, { "kind" => "outer" }),
            MemorySpaceId::CustomInvocation {
                transition_id,
                caller_thread_id,
                custom_id,
            } => serialize_object!(serializer, {
                "kind" => "custom_invocation",
                "transition_id" => DisplayValue(transition_id),
                "caller_thread_id" => DisplayValue(caller_thread_id),
                "custom_id" => custom_id.get(),
            }),
        }
    }
}

pub struct MemoryLocationProjection<'a>(pub &'a MemoryLocationId);

impl Serialize for MemoryLocationProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "space" => MemorySpaceProjection(self.0.space),
            "address" => DisplayValue(&self.0.address),
        })
    }
}

pub struct VmEventSequence<'a>(pub &'a [VmEvent]);

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

pub struct VmEventProjection<'a>(pub &'a VmEvent);

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
                "scope" => ScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "cell" => StaticCellProjection(*cell),
            }),
            VmEvent::InputConsumed {
                scope,
                thread_id,
                value,
            } => serialize_object!(serializer, {
                "kind" => "input_consumed",
                "scope" => ScopeProjection(*scope),
                "thread_id" => DisplayValue(*thread_id),
                "value" => value,
            }),
            VmEvent::RegisterChanged {
                scope,
                register,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "register_changed",
                "scope" => ScopeProjection(*scope),
                "register" => register,
                "old" => old,
                "new" => new,
            }),
            VmEvent::MemoryChanged {
                scope,
                location,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "memory_changed",
                "scope" => ScopeProjection(*scope),
                "location" => MemoryLocationProjection(location),
                "old" => old,
                "new" => new,
            }),
            VmEvent::CodeChanged {
                scope,
                cell,
                old,
                new,
            } => serialize_object!(serializer, {
                "kind" => "code_changed",
                "scope" => ScopeProjection(*scope),
                "cell" => StaticCellProjection(*cell),
                "old" => old.map(|primary| primary.token()),
                "new" => new.map(|primary| primary.token()),
            }),
            VmEvent::ThreadChanged {
                scope,
                before,
                after,
            } => {
                let code_grid = match scope {
                    ExecutionScope::Outer => CodeGridId::Outer,
                    ExecutionScope::Custom { custom_id, .. } => CodeGridId::Custom(*custom_id),
                };
                let before = ThreadProjection::from_fields(
                    code_grid,
                    before.id,
                    before.board,
                    before.position,
                    before.direction,
                    before.register_pointer,
                    before.status_flag,
                    before.private_registers,
                    &before.page,
                    &before.data_stack,
                    before.instruction_stack.iter().map(|item| item.code()),
                    before.call_stack.clone(),
                    before.phase,
                    before.random_state,
                );
                let after = ThreadProjection::from_fields(
                    code_grid,
                    after.id,
                    after.board,
                    after.position,
                    after.direction,
                    after.register_pointer,
                    after.status_flag,
                    after.private_registers,
                    &after.page,
                    &after.data_stack,
                    after.instruction_stack.iter().map(|item| item.code()),
                    after.call_stack.clone(),
                    after.phase,
                    after.random_state,
                );
                serialize_object!(serializer, {
                    "kind" => "thread_changed",
                    "scope" => ScopeProjection(*scope),
                    "before" => before,
                    "after" => after,
                })
            }
        }
    }
}

pub struct RuntimeMetricsProjection<'a>(pub &'a RuntimeMetrics);

impl Serialize for RuntimeMetricsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "committed_steps" => DisplayValue(metrics.committed_steps()),
        })
    }
}

pub struct RuntimeMetricSummaryProjection<'a>(pub &'a RuntimeMetricSummary);

impl Serialize for RuntimeMetricSummaryProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metrics = self.0;
        serialize_object!(serializer, {
            "committed_steps" => DisplayValue(metrics.committed_steps()),
        })
    }
}

pub struct RuntimeErrorSequence<'a>(pub &'a [RuntimeError]);

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

pub struct RuntimeErrorProjection<'a>(pub &'a RuntimeError);

impl Serialize for RuntimeErrorProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_object!(serializer, {
            "code" => self.0.code(),
            "error_number" => crate::error_number("vm", self.0.code()),
            "transition_id" => DisplayValue(self.0.transition_id()),
            "scope" => ScopeProjection(self.0.scope()),
            "details" => RuntimeErrorDetailsProjection(self.0.kind()),
        })
    }
}

pub struct RuntimeErrorDetailsProjection<'a>(pub &'a RuntimeErrorKind);

impl Serialize for RuntimeErrorDetailsProjection<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            RuntimeErrorKind::ConcurrentCallerStackReadConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCallerStackReadWriteConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCallerStackWriteConflict {
                internal_thread_ids,
            } => serialize_object!(serializer, {
                "internal_thread_ids" => ThreadIdSequence(internal_thread_ids),
            }),
            RuntimeErrorKind::ConcurrentCodeWriteConflict { cell, thread_ids } => {
                serialize_object!(serializer, {
                    "cell" => StaticCellProjection(*cell),
                    "thread_ids" => ThreadIdSequence(thread_ids),
                })
            }
            RuntimeErrorKind::ConcurrentInputConflict { thread_ids }
            | RuntimeErrorKind::ConcurrentOutputConflict { thread_ids } => {
                serialize_object!(serializer, {
                    "thread_ids" => ThreadIdSequence(thread_ids),
                })
            }
            RuntimeErrorKind::ConcurrentMemoryWriteConflict {
                address,
                thread_ids,
            } => serialize_object!(serializer, {
                "address" => DisplayValue(address),
                "thread_ids" => ThreadIdSequence(thread_ids),
            }),
            RuntimeErrorKind::ConcurrentWriteConflict {
                register,
                thread_ids,
            } => serialize_object!(serializer, {
                "register" => register,
                "thread_ids" => ThreadIdSequence(thread_ids),
            }),
            RuntimeErrorKind::ReturnWithoutCall {
                thread_id,
                board,
                position,
            } => serialize_object!(serializer, {
                "thread_id" => DisplayValue(thread_id),
                "board" => BoardIdProjection(*board),
                "position" => CoordinateProjection(*position),
            }),
        }
    }
}

pub struct ThreadIdSequence<'a>(pub &'a [u64]);

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

pub struct OptionalFaultProjection(pub Option<VmFault>);

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

pub struct FaultProjection(pub VmFault);

impl Serialize for FaultProjection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0 {
            VmFault::MetricCounterOverflow(counter) => serialize_object!(serializer, {
                "kind" => "metric_counter_overflow", "error_number" => crate::error_number("fault", "metric_counter_overflow"),
                "counter" => metric_counter_name(counter),
            }),
            VmFault::CommittedStepOverflow => {
                serialize_object!(serializer, { "kind" => "committed_step_overflow", "error_number" => crate::error_number("fault", "committed_steps_overflow") })
            }
            VmFault::InternalInvariantViolation => {
                serialize_object!(serializer, { "kind" => "internal_invariant_violation", "error_number" => crate::error_number("fault", "internal_invariant_violation") })
            }
        }
    }
}

pub fn direction_name(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
        Direction::Left => "left",
        Direction::Right => "right",
    }
}

pub fn vm_status_name(status: VmStatus) -> &'static str {
    match status {
        VmStatus::Running => "running",
        VmStatus::Halted => "halted",
        VmStatus::Error => "error",
    }
}

pub fn metric_counter_name(counter: MetricCounterOverflow) -> &'static str {
    match counter {
        MetricCounterOverflow::OperationCount => "operation_count",
        MetricCounterOverflow::CustomInternalDispatches => "custom_internal_dispatches",
        MetricCounterOverflow::CommittedSteps => "committed_steps",
        MetricCounterOverflow::DataStackUsage => "data_stack_usage",
        MetricCounterOverflow::InstructionStackUsage => "instruction_stack_usage",
        MetricCounterOverflow::CallStackUsage => "call_stack_usage",
    }
}

#[cfg(test)]
mod tests {
    use super::{metric_counter_name, MemoryLocationProjection, ProgramViewProjection};
    use crate::{MemoryLocationId, MemorySpaceId, ProgramView};
    use codegrid_model::Slot;
    use codegrid_vm::MetricCounterOverflow;

    #[test]
    fn custom_dispatch_counter_overflow_has_a_neutral_projection_name() {
        assert_eq!(
            metric_counter_name(MetricCounterOverflow::CustomInternalDispatches),
            "custom_internal_dispatches"
        );
    }

    #[test]
    fn custom_memory_projection_uses_neutral_transition_identity() {
        let location = MemoryLocationId {
            space: MemorySpaceId::CustomInvocation {
                transition_id: 42,
                caller_thread_id: 7,
                custom_id: Slot::new(2).expect("ID is in range"),
            },
            address: "-17".parse().expect("address is a valid BigInt"),
        };

        let value = serde_json::to_value(MemoryLocationProjection(&location))
            .expect("memory projection must serialize");

        assert_eq!(
            value,
            serde_json::json!({
                "space": {
                    "kind": "custom_invocation",
                    "transition_id": "42",
                    "caller_thread_id": "7",
                    "custom_id": 2
                },
                "address": "-17"
            })
        );
    }

    #[test]
    fn program_view_dimensions_serialize_as_json_numbers_for_main_and_function_boards() {
        let source = "@main\n@size 4x1\n~> [0 _ _\n\
@F0\n@size 2x1\n~v ]\n@end F0\n@end main\n";
        let program = codegrid_compiler::compile(source)
            .expect("program with a differently sized function board should compile");
        let view = ProgramView::from_verified(&program);

        let value = serde_json::to_value(ProgramViewProjection(&view))
            .expect("program view must serialize");

        assert_eq!(value["outer"]["main"]["width"], 4);
        assert!(value["outer"]["main"]["width"].is_number());
        assert_eq!(value["outer"]["main"]["height"], 1);
        assert!(value["outer"]["main"]["height"].is_number());
        assert_eq!(value["outer"]["functions"]["0"]["width"], 2);
        assert!(value["outer"]["functions"]["0"]["width"].is_number());
        assert_eq!(value["outer"]["functions"]["0"]["height"], 1);
        assert!(value["outer"]["functions"]["0"]["height"].is_number());
    }
}
