use codegrid_compiler::compile;
use codegrid_ir::BoardId;
use codegrid_model::{PrimaryInstruction, Slot};
use codegrid_vm::{Coordinate, InstructionKind, RunOutcome, Vm, VmConfig, VmStatus};
use std::num::NonZeroU64;

fn machine(source: &str, input: &[u8]) -> Vm {
    Vm::new(
        compile(source).expect("valid register-call source"),
        input.iter().copied(),
        VmConfig::new(0),
    )
    .unwrap()
}
fn folded(main: &str, definitions: &[(u8, &str)]) -> String {
    let width = main.split_whitespace().count();
    let mut source = format!("@main\n{main}\n");
    for (slot, contents) in definitions {
        let mut cells: Vec<_> = contents.split_whitespace().collect();
        cells.resize(width, "_");
        source.push_str(&format!("@M{slot}\n{}\n@end M{slot}\n", cells.join(" ")));
    }
    source.push_str("@end main\n");
    source
}

#[test]
fn register_calls_read_the_selected_pointer_and_function_bank() {
    let source = "@main\n~> } } } , [r . ;\n@F6\n~> + . ]\n@end F6\n@end main\n";
    let mut vm = machine(source, &[6]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.output, [7, 6]);
    assert_eq!(state.registers[3], 6);
    assert_eq!(state.registers[0], 0);
    assert_eq!(state.threads[0].register_pointer, 3);
    assert!(state.threads[0].call_stack.is_empty());
    assert_eq!(state.committed_steps, 13);
    let mut vm = machine(&folded("~> } } } , $r . ;", &[(6, ".6")]), &[6]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [6, 6]);
}

#[test]
fn folded_register_repeat_rereads_values_and_changed_pointers() {
    let mut vm = machine(
        &folded("~> , $rx3 . ;", &[(1, "+"), (2, "+"), (3, ".3")]),
        &[1],
    );
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [3, 3]);
    assert_eq!(vm.snapshot().committed_steps, 8);
    let mut vm = machine(
        &folded("~> , } , { $rx2 . ;", &[(1, "}"), (2, ".2")]),
        &[1, 2],
    );
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [2, 2]);
    assert_eq!(vm.snapshot().threads[0].register_pointer, 1);
}

#[test]
fn register_repeat_captures_count_once_but_rereads_the_fold_target() {
    let mut vm = machine(&folded("~> , $rxr . ;", &[(2, "+"), (3, ".3")]), &[2]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [3, 3]);
    assert_eq!(vm.snapshot().committed_steps, 7);
    // A later out-of-range target consumes one pass without a FoldResume.
    let mut vm = machine(&folded("~> , $rxr . ;", &[(2, "$!")]), &[2]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [253]);
    assert_eq!(vm.snapshot().committed_steps, 6);
}

#[test]
fn out_of_range_calls_skip_without_changing_registers_stacks_or_flag() {
    for value in [10, 255] {
        let mut vm = machine("~> , ( , [r $r . ;", &[value]);
        for _ in 0..4 {
            assert_eq!(vm.step().status, VmStatus::Running);
        }
        let before = vm.snapshot();
        let call = vm.prepare_step();
        assert_eq!(
            call.report().footprint.operations[&InstructionKind::CallRegister],
            1
        );
        call.commit();
        vm.step();
        let after = vm.snapshot();
        assert_eq!(after.registers, before.registers);
        assert_eq!(after.threads[0].data_stack, before.threads[0].data_stack);
        assert_eq!(after.threads[0].status_flag, 1);
        assert!(after.threads[0].call_stack.is_empty());
        assert_eq!(after.threads[0].position, Coordinate { x: 6, y: 0 });
        assert_eq!(vm.run(4), RunOutcome::Halted);
        assert_eq!(vm.snapshot().output, [value]);
    }
    let mut vm = machine("~> , $rx3 . ;", &[255]);
    assert_eq!(vm.run(10), RunOutcome::Halted);
    assert_eq!(vm.snapshot().committed_steps, 7);
}

#[test]
fn false_prefixes_skip_target_resolution_and_call_operations() {
    let mut vm = machine("~> , ?0[r ?0$r . ;", &[255]);
    vm.step();
    vm.step();
    for _ in 0..2 {
        let prepared = vm.prepare_step();
        assert_eq!(prepared.report().footprint.operations.len(), 1);
        assert_eq!(
            prepared.report().footprint.operations[&InstructionKind::Condition],
            1
        );
        prepared.commit();
    }
    assert_eq!(vm.run(4), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [255]);
}

#[test]
fn empty_fold_returns_but_empty_functions_keep_wrapping() {
    for source in ["~> $r ;".to_owned(), folded("~> $r ;", &[(0, "_")])] {
        let mut vm = machine(&source, &[]);
        assert_eq!(vm.run(10), RunOutcome::Halted);
        assert_eq!(vm.snapshot().committed_steps, 4);
    }
    for (source, width) in [
        ("~> , [r ;", 4),
        ("@main\n~> , [r ;\n@F6\n_ _ _\n@end F6\n@end main\n", 3),
    ] {
        let mut vm = machine(source, &[6]);
        assert_eq!(vm.run(20), RunOutcome::StepLimitReached);
        let state = vm.snapshot();
        assert_eq!(
            state.threads[0].board,
            BoardId::Function(Slot::new(6).unwrap())
        );
        assert_eq!(
            state.threads[0].position,
            Coordinate {
                x: 17 % width,
                y: 0
            }
        );
        assert_eq!(state.threads[0].call_stack.len(), 1);
        assert!(vm.set_thread_execution_location(
            0,
            Coordinate { x: width - 1, y: 0 },
            codegrid_model::Direction::Right
        ));
        vm.step();
        assert_eq!(vm.snapshot().threads[0].position, Coordinate { x: 0, y: 0 });
    }
}

#[test]
fn call_targets_are_local_to_the_current_custom_codegrid() {
    let source = "@main\n~> , ( #0 ! ) . ;\n@F6\n~> .9 ]\n@end F6\n@end main\n@C0\n~> , [r #]\n@F6\n~> .6 ]\n@end F6\n@end C0\n";
    let mut vm = machine(source, &[6]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [6]);
}

#[test]
fn register_call_recursion_never_reuses_a_static_tail_frame() {
    let source = "@main\n~> [r ;\n@F0\n~> [r ]\n@end F0\n@end main\n";
    let program = compile(source).unwrap();
    assert!(program.tail_call_sites().is_empty());
    let mut vm = machine(source, &[]);
    assert_eq!(vm.run(20), RunOutcome::StepLimitReached);
    assert_eq!(vm.snapshot().threads[0].call_stack.len(), 10);
}

#[test]
fn read_code_uses_dynamic_opcode_on_resume_and_invalid_target_skip() {
    for (source, input, expected) in [
        ("~> , [r* % . ;".to_owned(), 10, 205),
        (
            "@main\n~> , [r* % . ;\n@F1\n~> + ]\n@end F1\n@end main\n".to_owned(),
            1,
            205,
        ),
        (folded("~> , $r* % . ;", &[(1, "+")]), 1, 150),
    ] {
        let mut vm = machine(&source, &[input]);
        assert_eq!(vm.run(30), RunOutcome::Halted);
        assert_eq!(vm.snapshot().output, [expected]);
    }
}

#[test]
fn dynamic_call_write_code_is_deferred_until_return() {
    let source = "@main\n~> , & ! [r= . ;\n@F0\n~> ]\n@end F0\n@end main\n";
    let mut vm = machine(source, &[33]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(
        vm.snapshot().runtime_program.main.cells[4].primary,
        Some(PrimaryInstruction::Clear)
    );
    assert_eq!(vm.snapshot().output, [0]);
}

#[test]
fn empty_dynamic_function_in_custom_respects_work_limits_and_rolls_back() {
    let source = "@main\n~> #0 ;\n@end main\n@C0\n[r\n@end C0\n";
    let mut vm = machine(source, &[]);
    vm.step();
    let before = vm.snapshot();
    assert!(vm
        .prepare_step_with_work_limit(NonZeroU64::new(50).unwrap())
        .is_err());
    assert_eq!(vm.snapshot(), before);
}

#[test]
fn concurrent_dynamic_function_output_conflicts_roll_back_the_whole_tick() {
    let source = "@main\n~> [r _ _ ~> [r _ _\n@F0\n~> . ]\n@end F0\n@end main\n";
    let mut vm = machine(source, &[]);
    for _ in 0..3 {
        vm.step();
    }
    let before = vm.snapshot();
    assert_eq!(vm.step().status, VmStatus::Error);
    let after = vm.snapshot();
    assert_eq!(after.committed_steps, 3);
    assert_eq!(after.threads, before.threads);
    assert!(after.output.is_empty());
}

#[test]
fn dynamic_syntax_attachment_and_folded_context_boundaries() {
    for atom in [
        "$r", "$rx3", "$rxr", "$r*", "$r=", "[r", "[r*", "[r=", "?0[r", "?!$r",
    ] {
        compile(&format!("~> {atom} ;")).unwrap();
    }
    for atom in ["$R", "[R", "$r0", "[r0", "[rx2", "[rxr"] {
        assert!(compile(&format!("~> {atom} ;")).is_err(), "{atom}");
    }
    for atom in ["$r", "[r"] {
        let errors = compile(&folded("~> $0 ;", &[(0, atom)])).unwrap_err();
        assert!(errors.iter().any(|d| d.code == "source.fold_primary"));
    }
}

#[test]
fn nested_register_call_reads_the_private_function_bank() {
    let source = "@main\n~> [0 . ;\n@F0\n~> + [r . ]\n@end F0\n@F1\n~> .7 ]\n@end F1\n@end main\n";
    let mut vm = machine(source, &[]);
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, [7, 1, 0]);
}

#[test]
fn dynamic_targets_use_the_same_transition_start_snapshot_as_siblings() {
    let source = "@main\n~> [r _ + ~<\n@F0\n~> .1 ]\n@end F0\n@F1\n~> .2 ]\n@end F1\n@end main\n";
    let mut vm = machine(source, &[]);
    vm.step();
    vm.step();
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 1);
    assert_eq!(
        state.threads[0].board,
        BoardId::Function(Slot::new(0).unwrap())
    );
    assert_eq!(state.threads[0].private_registers.unwrap()[0], 0);
}

#[test]
fn write_code_cannot_install_either_register_call_inside_a_fold() {
    for code in [150, 205] {
        let mut vm = machine(&folded("~> , & $0 % . ;", &[(0, "+=")]), &[code]);
        vm.step();
        vm.step();
        vm.step();
        let prepared = vm.prepare_step();
        assert!(!prepared
            .report()
            .footprint
            .operations
            .contains_key(&InstructionKind::WriteCode));
        prepared.commit();
        let state = vm.snapshot();
        assert_eq!(state.threads[0].instruction_stack.len(), 1);
        assert_eq!(
            state.runtime_program.main.folded_blocks[&Slot::new(0).unwrap()].cells[0],
            Some(PrimaryInstruction::Add)
        );
        assert_eq!(vm.run(20), RunOutcome::Halted);
        assert_eq!(vm.snapshot().output, [code]);
    }
    let mut vm = machine("~> $rxr ;", &[]);
    assert_eq!(vm.run(5), RunOutcome::Halted);
    assert_eq!(vm.snapshot().committed_steps, 3);
}
