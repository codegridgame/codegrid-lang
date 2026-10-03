use std::collections::VecDeque;

use codegrid_ir::{Board, BoardId, CodeGridId, ScopedProgram, VerifiedProgram};
use codegrid_model::{
    AttachmentInstruction, Direction, InstructionStackItem, PageDirection, PointerDirection,
    PrimaryInstruction, ShiftDirection, Slot, Value,
};
use num_bigint::BigInt;

use crate::effects::{
    resolve_shared_conflicts, CallerStackRead, CallerStackWrite, CodeWrite, InputRead, MemoryWrite,
    OutputWrite, RegisterWrite,
};
use crate::thread::{CallFrame, ExecutionPhase, ThreadState};
use crate::{
    custom_invocation_seed, effective_address, internal_thread_state, move_folded, move_normal,
    Coordinate, ExecutionScope, FoldStep, InstructionKind, Memory, MemoryLocationId, MemorySpaceId,
    MetricCounterOverflow, RuntimeError, RuntimeErrorKind, StaticCellId, VmConfig, VmFault,
};

use super::{
    add_stack_usage, commit_effects, committed_state_events, record_cell_visit, record_operation,
    runtime_board, runtime_cell, stack_usage_with_growth, ExecutionMode, ExecutionWorkBudget,
    StackUsage, TickDraft,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flow {
    Move,
    Stay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CustomOutcome {
    Returned,
    Halted,
    Failed,
    WorkLimit,
}

fn accumulate_custom_stack_usage(
    draft: &mut TickDraft,
    internal_tick: u64,
    usage: StackUsage,
) -> Result<(), VmFault> {
    let accumulated = draft.custom_stack_samples.entry(internal_tick).or_default();
    *accumulated = add_stack_usage(*accumulated, usage)?;
    Ok(())
}

fn record_custom_caller_stack_sample(
    draft: &mut TickDraft,
    internal_tick: u64,
    caller_thread_id: u64,
    stack_before: &[Value],
    staged_pushes: usize,
) -> Result<(), VmFault> {
    let base = u64::try_from(stack_before.len())
        .map_err(|_| VmFault::MetricCounterOverflow(MetricCounterOverflow::DataStackUsage))?;
    let pushes = u64::try_from(staged_pushes)
        .map_err(|_| VmFault::MetricCounterOverflow(MetricCounterOverflow::DataStackUsage))?;
    let sampled_depth = base
        .checked_add(pushes)
        .ok_or(VmFault::MetricCounterOverflow(
            MetricCounterOverflow::DataStackUsage,
        ))?;
    draft
        .custom_caller_stack_samples
        .entry(internal_tick)
        .or_default()
        .entry(caller_thread_id)
        .and_modify(|depth| *depth = (*depth).max(sampled_depth))
        .or_insert(sampled_depth);
    Ok(())
}

pub(super) fn evaluate_thread(
    thread: &mut ThreadState,
    tick: u64,
    registers: &[Value; 10],
    memory: &Memory,
    input: &VecDeque<Value>,
    caller_stack: Option<&[Value]>,
    runtime: &ScopedProgram,
    verified_program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
    budget: &mut ExecutionWorkBudget,
) {
    if thread.phase == ExecutionPhase::Terminated {
        if mode.scope == ExecutionScope::Outer {
            draft.fault = Some(VmFault::InternalInvariantViolation);
        }
        return;
    }
    if !budget.consume() {
        draft.work_limit_exceeded = true;
        return;
    }

    match thread.phase {
        ExecutionPhase::Normal => execute_normal(
            thread,
            tick,
            registers,
            memory,
            input,
            caller_stack,
            runtime,
            verified_program,
            config,
            metrics,
            draft,
            mode,
            budget,
        ),
        ExecutionPhase::AfterCall => {
            execute_after_call(thread, tick, runtime, config, metrics, draft, mode)
        }
        ExecutionPhase::Repeat { total, completed } => execute_repeat(
            thread,
            tick,
            registers,
            memory,
            input,
            caller_stack,
            runtime,
            verified_program,
            config,
            metrics,
            draft,
            mode,
            total,
            completed,
            budget,
        ),
        ExecutionPhase::Fold {
            fold_id,
            internal_position,
            internal_direction,
            saved_outer_direction,
        } => execute_fold_cell(
            thread,
            tick,
            registers,
            memory,
            input,
            caller_stack,
            runtime,
            verified_program,
            config,
            metrics,
            draft,
            mode,
            fold_id,
            internal_position,
            internal_direction,
            saved_outer_direction,
            budget,
        ),
        ExecutionPhase::FoldResume {
            saved_outer_direction,
        } => {
            thread.direction = saved_outer_direction;
            thread.phase = ExecutionPhase::Normal;
            move_normal_cell(thread, tick, runtime, config, draft, mode);
        }
        ExecutionPhase::Terminated => unreachable!("terminated threads are skipped"),
    }
}

fn execute_normal(
    thread: &mut ThreadState,
    tick: u64,
    registers: &[Value; 10],
    memory: &Memory,
    input: &VecDeque<Value>,
    caller_stack: Option<&[Value]>,
    runtime: &ScopedProgram,
    program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
    budget: &mut ExecutionWorkBudget,
) {
    let Some(cell) = runtime_cell(runtime, thread.board, thread.position).cloned() else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    let id = cell_id(mode.code_grid, thread.board, None, thread.position);
    record_cell_visit(metrics, draft, thread, id, mode.scope);
    let Some(primary) = cell.primary else {
        if let Some(
            attachment @ (AttachmentInstruction::ReadCode | AttachmentInstruction::WriteCode),
        ) = cell.attachment
        {
            execute_attachment(thread, attachment, id, None, metrics, draft);
            if draft.fault.is_some() {
                return;
            }
        } else if cell.attachment.is_some() {
            draft.fault = Some(VmFault::InternalInvariantViolation);
            return;
        }
        move_normal_cell(thread, tick, runtime, config, draft, mode);
        return;
    };

    record_primary(primary, metrics, draft);
    if draft.fault.is_some() {
        return;
    }
    let flow = execute_primary(
        thread,
        primary,
        tick,
        registers,
        memory,
        input,
        caller_stack,
        runtime,
        program,
        config,
        metrics,
        draft,
        mode,
        budget,
    );
    if draft.fault.is_some() || flow != Flow::Move {
        return;
    }

    match cell.attachment {
        Some(AttachmentInstruction::Repeat(total)) => {
            thread.phase = ExecutionPhase::Repeat {
                total,
                completed: 1,
            };
            return;
        }
        Some(attachment) => {
            execute_attachment(thread, attachment, id, cell.primary, metrics, draft);
            if draft.fault.is_some() {
                return;
            }
        }
        None => {}
    }
    move_current(thread, tick, runtime, config, draft, mode);
}

fn execute_repeat(
    thread: &mut ThreadState,
    tick: u64,
    registers: &[Value; 10],
    memory: &Memory,
    input: &VecDeque<Value>,
    caller_stack: Option<&[Value]>,
    runtime: &ScopedProgram,
    program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
    total: u8,
    completed: u8,
    budget: &mut ExecutionWorkBudget,
) {
    let Some(cell) = runtime_cell(runtime, thread.board, thread.position).cloned() else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    let Some(primary) = cell.primary else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    if cell.attachment != Some(AttachmentInstruction::Repeat(total)) || completed >= total {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    }
    record_cell_visit(
        metrics,
        draft,
        thread,
        cell_id(mode.code_grid, thread.board, None, thread.position),
        mode.scope,
    );
    record_primary(primary, metrics, draft);
    if draft.fault.is_some() {
        return;
    }
    let flow = execute_primary(
        thread,
        primary,
        tick,
        registers,
        memory,
        input,
        caller_stack,
        runtime,
        program,
        config,
        metrics,
        draft,
        mode,
        budget,
    );
    if draft.fault.is_some() || flow != Flow::Move {
        return;
    }
    let completed = completed + 1;
    if completed == total {
        thread.phase = ExecutionPhase::Normal;
        move_current(thread, tick, runtime, config, draft, mode);
    } else {
        thread.phase = ExecutionPhase::Repeat { total, completed };
    }
}

fn execute_after_call(
    thread: &mut ThreadState,
    tick: u64,
    runtime: &ScopedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
) {
    let Some(cell) = runtime_cell(runtime, thread.board, thread.position).cloned() else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    if let Some(attachment @ (AttachmentInstruction::ReadCode | AttachmentInstruction::WriteCode)) =
        cell.attachment
    {
        execute_attachment(
            thread,
            attachment,
            cell_id(mode.code_grid, thread.board, None, thread.position),
            cell.primary,
            metrics,
            draft,
        );
    }
    if draft.fault.is_some() {
        return;
    }
    thread.phase = ExecutionPhase::Normal;
    move_normal_cell(thread, tick, runtime, config, draft, mode);
}

fn execute_fold_cell(
    thread: &mut ThreadState,
    tick: u64,
    registers: &[Value; 10],
    memory: &Memory,
    input: &VecDeque<Value>,
    caller_stack: Option<&[Value]>,
    runtime: &ScopedProgram,
    program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
    fold_id: Slot,
    position: Coordinate,
    direction: Direction,
    saved_outer_direction: Direction,
    budget: &mut ExecutionWorkBudget,
) {
    let Some(board) = runtime_board(runtime, thread.board) else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    let Some(block) = board.folded_blocks.get(&fold_id) else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    let Some(primary) = block.cells.get(position.x).copied() else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    thread.direction = direction;
    record_cell_visit(
        metrics,
        draft,
        thread,
        cell_id(mode.code_grid, thread.board, Some(fold_id), position),
        mode.scope,
    );
    let flow = if let Some(primary) = primary {
        record_primary(primary, metrics, draft);
        if draft.fault.is_some() {
            return;
        }
        execute_primary(
            thread,
            primary,
            tick,
            registers,
            memory,
            input,
            caller_stack,
            runtime,
            program,
            config,
            metrics,
            draft,
            mode,
            budget,
        )
    } else {
        Flow::Move
    };
    if draft.fault.is_some() || flow != Flow::Move {
        return;
    }
    let next_direction = thread.direction;
    match move_folded(position, next_direction, block.cells.len()) {
        Some(FoldStep::Moved(next)) => {
            thread.phase = ExecutionPhase::Fold {
                fold_id,
                internal_position: next,
                internal_direction: next_direction,
                saved_outer_direction,
            };
        }
        Some(FoldStep::VerticalExit(_)) => {
            thread.direction = saved_outer_direction;
            thread.phase = ExecutionPhase::FoldResume {
                saved_outer_direction,
            };
        }
        None => draft.fault = Some(VmFault::InternalInvariantViolation),
    }
}

fn execute_primary(
    thread: &mut ThreadState,
    primary: PrimaryInstruction,
    tick: u64,
    registers: &[Value; 10],
    memory: &Memory,
    input: &VecDeque<Value>,
    caller_stack: Option<&[Value]>,
    runtime: &ScopedProgram,
    program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
    mode: ExecutionMode,
    budget: &mut ExecutionWorkBudget,
) -> Flow {
    let value = registers[usize::from(thread.register_pointer)];
    match primary {
        PrimaryInstruction::Direction(direction) => thread.direction = direction,
        PrimaryInstruction::RandomDirection => thread.direction = thread.rng.next_direction(),
        PrimaryInstruction::IfZero(direction) => {
            if value == 0 {
                thread.direction = direction;
            }
        }
        PrimaryInstruction::Read(direction) => {
            if mode.scope == ExecutionScope::Outer {
                let value = input.front().copied();
                draft.effects.input_reads.push(InputRead {
                    thread_id: thread.id,
                    register: thread.register_pointer,
                    value,
                });
                if value.is_none() {
                    thread.direction = direction;
                }
            } else {
                let value = caller_stack.and_then(|stack| stack.last().copied());
                draft.effects.caller_stack_reads.push(CallerStackRead {
                    thread_id: thread.id,
                    register: thread.register_pointer,
                    value,
                });
                if value.is_none() {
                    thread.direction = direction;
                }
            }
        }
        PrimaryInstruction::Clear => stage_register(thread, 0, draft),
        PrimaryInstruction::Add => stage_register(thread, value.wrapping_add(1), draft),
        PrimaryInstruction::Sub => stage_register(thread, value.wrapping_sub(1), draft),
        PrimaryInstruction::MoveRegisterPointer(direction) => {
            thread.register_pointer = match direction {
                PointerDirection::Left => (thread.register_pointer + 9) % 10,
                PointerDirection::Right => (thread.register_pointer + 1) % 10,
            };
        }
        PrimaryInstruction::Output | PrimaryInstruction::OutputImmediate(_) => {
            let value = match primary {
                PrimaryInstruction::OutputImmediate(digit) => digit.get(),
                _ => value,
            };
            if mode.scope == ExecutionScope::Outer {
                draft.effects.output_writes.push(OutputWrite {
                    thread_id: thread.id,
                    value,
                });
            } else {
                draft.effects.caller_stack_writes.push(CallerStackWrite {
                    thread_id: thread.id,
                    value,
                });
            }
        }
        PrimaryInstruction::Push => thread.data_stack.push(value),
        PrimaryInstruction::PopAdd => {
            if let Some(popped) = thread.data_stack.pop() {
                stage_register(thread, value.wrapping_add(popped), draft);
            }
        }
        PrimaryInstruction::Decode => {
            if let Some(item) = InstructionStackItem::from_code(value) {
                thread.instruction_stack.push(item);
            }
        }
        PrimaryInstruction::Encode => {
            if let Some(item) = thread.instruction_stack.pop() {
                let code = match item {
                    InstructionStackItem::Empty => 32,
                    InstructionStackItem::Primary(primary) => primary.code(),
                };
                stage_register(thread, code, draft);
            }
        }
        PrimaryInstruction::Call(function) => {
            let current_function = match thread.board {
                BoardId::Function(current) => Some(current),
                BoardId::Main => None,
            };
            let Some(board) = runtime_board(runtime, thread.board) else {
                draft.fault = Some(VmFault::InternalInvariantViolation);
                return Flow::Stay;
            };
            let Some(index) = thread
                .position
                .y
                .checked_mul(board.width)
                .and_then(|base| base.checked_add(thread.position.x))
            else {
                draft.fault = Some(VmFault::InternalInvariantViolation);
                return Flow::Stay;
            };
            let is_tail = current_function == Some(function)
                && program.is_tail_call(mode.code_grid, function, index, config.boundary_mode());
            let Some(target) = runtime.functions.get(&function) else {
                draft.fault = Some(VmFault::InternalInvariantViolation);
                return Flow::Stay;
            };
            let Some((position, direction)) = first_entry(target) else {
                draft.fault = Some(VmFault::InternalInvariantViolation);
                return Flow::Stay;
            };
            if !is_tail {
                thread.call_stack.push(CallFrame {
                    caller_board: thread.board,
                    call_position: thread.position,
                    saved_direction: thread.direction,
                });
            }
            thread.board = BoardId::Function(function);
            thread.position = position;
            thread.direction = direction;
            return Flow::Stay;
        }
        PrimaryInstruction::Return => {
            let Some(frame) = thread.call_stack.pop() else {
                draft.errors.push(RuntimeError::new(
                    tick,
                    mode.scope,
                    RuntimeErrorKind::ReturnWithoutCall {
                        thread_id: thread.id,
                        board: thread.board,
                        position: thread.position,
                    },
                ));
                return Flow::Stay;
            };
            thread.board = frame.caller_board;
            thread.position = frame.call_position;
            thread.direction = frame.saved_direction;
            thread.phase = ExecutionPhase::AfterCall;
            return Flow::Stay;
        }
        PrimaryInstruction::Nand => {
            if let Some(popped) = thread.data_stack.pop() {
                stage_register(thread, !(value & popped), draft);
            }
        }
        PrimaryInstruction::MemoryLoad => {
            let address = effective_address(&thread.page, value);
            metrics.record_memory_access(memory_location(mode, tick, address.clone()));
            thread.data_stack.push(memory.read(&address));
        }
        PrimaryInstruction::MemoryStore => {
            if let Some(stored) = thread.data_stack.pop() {
                let address = effective_address(&thread.page, value);
                let location = memory_location(mode, tick, address);
                metrics.record_memory_access(location.clone());
                draft.effects.memory_writes.push(MemoryWrite {
                    thread_id: thread.id,
                    location,
                    value: stored,
                });
            }
        }
        PrimaryInstruction::MovePage(direction) => match direction {
            PageDirection::Increment => thread.page += BigInt::from(1u8),
            PageDirection::Decrement => thread.page -= BigInt::from(1u8),
        },
        PrimaryInstruction::Shift(direction) => stage_register(
            thread,
            match direction {
                ShiftDirection::Left => value.wrapping_shl(1),
                ShiftDirection::Right => value >> 1,
            },
            draft,
        ),
        PrimaryInstruction::FoldedBlock(fold_id) => {
            let saved_outer_direction = thread.direction;
            thread.phase = ExecutionPhase::Fold {
                fold_id,
                internal_position: Coordinate { x: 0, y: 0 },
                internal_direction: Direction::Right,
                saved_outer_direction,
            };
            execute_fold_cell(
                thread,
                tick,
                registers,
                memory,
                input,
                caller_stack,
                runtime,
                program,
                config,
                metrics,
                draft,
                mode,
                fold_id,
                Coordinate { x: 0, y: 0 },
                Direction::Right,
                saved_outer_direction,
                budget,
            );
            return Flow::Stay;
        }
        PrimaryInstruction::Custom(custom_id) => match execute_custom(
            thread, custom_id, tick, program, config, metrics, draft, budget,
        ) {
            CustomOutcome::Returned => {}
            CustomOutcome::Halted | CustomOutcome::Failed | CustomOutcome::WorkLimit => {
                return Flow::Stay;
            }
        },
        PrimaryInstruction::CustomReturn => {
            if mode.scope == ExecutionScope::Outer {
                draft.fault = Some(VmFault::InternalInvariantViolation);
            } else {
                thread.phase = ExecutionPhase::Terminated;
            }
            return Flow::Stay;
        }
        PrimaryInstruction::Halt => {
            draft.halt_requested = true;
            return Flow::Stay;
        }
    }
    Flow::Move
}

fn execute_attachment(
    thread: &mut ThreadState,
    attachment: AttachmentInstruction,
    cell: StaticCellId,
    tick_start_primary: Option<PrimaryInstruction>,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
) {
    let Some(kind) = InstructionKind::from_attachment(attachment) else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    record_operation(metrics, kind, draft);
    if draft.fault.is_some() {
        return;
    }
    match attachment {
        AttachmentInstruction::ReadCode => {
            let item = match tick_start_primary {
                Some(primary) => match InstructionStackItem::from_primary(primary) {
                    Some(item) => item,
                    None => {
                        draft.fault = Some(VmFault::InternalInvariantViolation);
                        return;
                    }
                },
                None => InstructionStackItem::Empty,
            };
            thread.instruction_stack.push(item);
        }
        AttachmentInstruction::WriteCode => {
            if let Some(item) = thread.instruction_stack.pop() {
                draft.effects.code_writes.push(CodeWrite {
                    thread_id: thread.id,
                    cell,
                    primary: item.primary(),
                });
            }
        }
        AttachmentInstruction::Repeat(_) => {
            draft.fault = Some(VmFault::InternalInvariantViolation);
        }
    }
}

fn execute_custom(
    caller: &mut ThreadState,
    custom_id: Slot,
    tick: u64,
    program: &VerifiedProgram,
    config: VmConfig,
    metrics: &mut crate::RuntimeMetrics,
    outer_draft: &mut TickDraft,
    budget: &mut ExecutionWorkBudget,
) -> CustomOutcome {
    let Some(definition) = program.program().customs.get(&custom_id) else {
        outer_draft.fault = Some(VmFault::InternalInvariantViolation);
        return CustomOutcome::Failed;
    };
    let mut runtime = definition.program.clone();
    let mut registers = [0; 10];
    let mut memory = Memory::default();
    let mut input = VecDeque::new();
    let invocation_seed = custom_invocation_seed(config.seed(), caller.id, tick, custom_id);
    let mut threads = initial_custom_threads(&runtime, invocation_seed);
    let limit = config.custom_execution_limit().get();

    for internal_tick in 1..=limit {
        let scope = ExecutionScope::Custom {
            caller_thread_id: caller.id,
            custom_id,
            internal_tick,
        };
        let registers_before = registers;
        let memory_before = memory.clone();
        let runtime_before = runtime.clone();
        let threads_before = threads.clone();
        let caller_stack_before = caller.data_stack.clone();
        let mut local_draft = TickDraft::default();
        let mode = ExecutionMode {
            code_grid: CodeGridId::Custom(custom_id),
            scope,
        };
        for thread in &mut threads {
            if thread.phase == ExecutionPhase::Terminated {
                continue;
            }
            evaluate_thread(
                thread,
                tick,
                &registers_before,
                &memory_before,
                &input,
                Some(&caller_stack_before),
                &runtime_before,
                program,
                config,
                metrics,
                &mut local_draft,
                mode,
                budget,
            );
            if local_draft.work_limit_exceeded {
                outer_draft.work_limit_exceeded = true;
                return CustomOutcome::WorkLimit;
            }
            if local_draft.fault.is_some() {
                outer_draft.fault = local_draft.fault;
                return CustomOutcome::Failed;
            }
        }
        local_draft.errors.extend(resolve_shared_conflicts(
            &local_draft.effects,
            false,
            !caller_stack_before.is_empty(),
            tick,
            scope,
        ));
        if let Err(fault) = record_custom_caller_stack_sample(
            outer_draft,
            internal_tick,
            caller.id,
            &caller_stack_before,
            local_draft.effects.caller_stack_writes.len(),
        ) {
            outer_draft.fault = Some(fault);
            return CustomOutcome::Failed;
        }
        if !local_draft.errors.is_empty() {
            let stack_usage = match stack_usage_with_growth(&threads_before, &threads) {
                Ok(usage) => usage,
                Err(fault) => {
                    outer_draft.fault = Some(fault);
                    return CustomOutcome::Failed;
                }
            };
            if let Err(fault) =
                accumulate_custom_stack_usage(outer_draft, internal_tick, stack_usage)
            {
                outer_draft.fault = Some(fault);
                return CustomOutcome::Failed;
            }
            outer_draft.errors.extend(local_draft.errors);
            return CustomOutcome::Failed;
        }

        let mut unused_output = Vec::new();
        commit_effects(
            &mut registers,
            &mut memory,
            &mut input,
            &mut unused_output,
            &mut runtime,
            &local_draft.effects,
        );
        if local_draft
            .effects
            .caller_stack_reads
            .iter()
            .any(|read| read.value.is_some())
        {
            caller.data_stack.pop();
        }
        caller.data_stack.extend(
            local_draft
                .effects
                .caller_stack_writes
                .iter()
                .map(|write| write.value),
        );

        let stack_usage = match stack_usage_with_growth(&threads_before, &threads) {
            Ok(usage) => usage,
            Err(fault) => {
                outer_draft.fault = Some(fault);
                return CustomOutcome::Failed;
            }
        };
        if let Err(fault) = accumulate_custom_stack_usage(outer_draft, internal_tick, stack_usage) {
            outer_draft.fault = Some(fault);
            return CustomOutcome::Failed;
        }
        outer_draft.events.extend(local_draft.events);
        outer_draft.committed_events.extend(committed_state_events(
            scope,
            &registers_before,
            &registers,
            &memory_before,
            &memory,
            &runtime_before,
            &runtime,
            &threads_before,
            &threads,
            &local_draft.effects,
        ));

        if local_draft.halt_requested {
            outer_draft.halt_requested = true;
            return CustomOutcome::Halted;
        }
        if threads
            .iter()
            .all(|thread| thread.phase == ExecutionPhase::Terminated)
        {
            return CustomOutcome::Returned;
        }
        if internal_tick == limit {
            outer_draft.errors.push(RuntimeError::new(
                tick,
                scope,
                RuntimeErrorKind::CustomExecutionLimitExceeded { limit },
            ));
            return CustomOutcome::Failed;
        }
    }
    outer_draft.fault = Some(VmFault::InternalInvariantViolation);
    CustomOutcome::Failed
}

fn initial_custom_threads(program: &ScopedProgram, invocation_seed: u64) -> Vec<ThreadState> {
    program
        .main
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            let direction = cell.entry?;
            let id = u64::try_from(index).ok()?;
            // The verifier guarantees an Entry and dimensions that fit the
            // portable index space. IDs are re-densified below for row order.
            Some((index, direction, id))
        })
        .enumerate()
        .map(|(ordinal, (index, direction, _))| {
            let id = ordinal as u64;
            ThreadState::initial(
                id,
                Coordinate {
                    x: index % program.main.width,
                    y: index / program.main.width,
                },
                direction,
                internal_thread_state(invocation_seed, id),
            )
        })
        .collect()
}

fn record_primary(
    primary: PrimaryInstruction,
    metrics: &mut crate::RuntimeMetrics,
    draft: &mut TickDraft,
) {
    if let Some(kind) = InstructionKind::from_primary(primary) {
        record_operation(metrics, kind, draft);
    }
}

fn first_entry(board: &Board) -> Option<(Coordinate, Direction)> {
    board.cells.iter().enumerate().find_map(|(index, cell)| {
        cell.entry.map(|direction| {
            (
                Coordinate {
                    x: index % board.width,
                    y: index / board.width,
                },
                direction,
            )
        })
    })
}

fn cell_id(
    code_grid: CodeGridId,
    board: BoardId,
    folded_block: Option<Slot>,
    position: Coordinate,
) -> StaticCellId {
    StaticCellId {
        code_grid,
        board,
        folded_block,
        position,
    }
}

fn memory_location(mode: ExecutionMode, tick: u64, address: BigInt) -> MemoryLocationId {
    let space = match (mode.code_grid, mode.scope) {
        (CodeGridId::Outer, _) => MemorySpaceId::Outer,
        (
            CodeGridId::Custom(_),
            ExecutionScope::Custom {
                caller_thread_id,
                custom_id,
                ..
            },
        ) => MemorySpaceId::CustomInvocation {
            global_tick: tick,
            caller_thread_id,
            custom_id,
        },
        _ => MemorySpaceId::Outer,
    };
    MemoryLocationId { space, address }
}

fn stage_register(thread: &ThreadState, value: Value, draft: &mut TickDraft) {
    draft.effects.register_writes.push(RegisterWrite {
        thread_id: thread.id,
        register: thread.register_pointer,
        value,
    });
}

fn move_current(
    thread: &mut ThreadState,
    tick: u64,
    runtime: &ScopedProgram,
    config: VmConfig,
    draft: &mut TickDraft,
    mode: ExecutionMode,
) {
    if let ExecutionPhase::Fold {
        fold_id,
        internal_position,
        internal_direction: _,
        saved_outer_direction,
    } = thread.phase
    {
        let Some(board) = runtime_board(runtime, thread.board) else {
            draft.fault = Some(VmFault::InternalInvariantViolation);
            return;
        };
        let Some(block) = board.folded_blocks.get(&fold_id) else {
            draft.fault = Some(VmFault::InternalInvariantViolation);
            return;
        };
        match move_folded(internal_position, thread.direction, block.cells.len()) {
            Some(FoldStep::Moved(next)) => {
                thread.phase = ExecutionPhase::Fold {
                    fold_id,
                    internal_position: next,
                    internal_direction: thread.direction,
                    saved_outer_direction,
                };
            }
            Some(FoldStep::VerticalExit(_)) => {
                thread.direction = saved_outer_direction;
                thread.phase = ExecutionPhase::FoldResume {
                    saved_outer_direction,
                };
            }
            None => draft.fault = Some(VmFault::InternalInvariantViolation),
        }
    } else {
        move_normal_cell(thread, tick, runtime, config, draft, mode);
    }
}

fn move_normal_cell(
    thread: &mut ThreadState,
    tick: u64,
    runtime: &ScopedProgram,
    config: VmConfig,
    draft: &mut TickDraft,
    mode: ExecutionMode,
) {
    let Some(board) = runtime_board(runtime, thread.board) else {
        draft.fault = Some(VmFault::InternalInvariantViolation);
        return;
    };
    if let Some(next) = move_normal(
        thread.position,
        thread.direction,
        board.width,
        board.height,
        config.boundary_mode(),
    ) {
        thread.position = next;
    } else {
        draft.errors.push(RuntimeError::new(
            tick,
            mode.scope,
            RuntimeErrorKind::OutOfBounds {
                thread_id: thread.id,
                board: thread.board,
                position: thread.position,
                direction: thread.direction,
            },
        ));
    }
}
