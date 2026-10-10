use codegrid_compiler::compile;
use codegrid_model::ConditionPrefix;
use codegrid_vm::{InstructionKind, RunOutcome, Vm, VmConfig, VmStatus};
use std::num::NonZeroU64;

fn machine(source: &str, input: &[u8]) -> Vm {
    Vm::new(
        compile(source).expect("valid conditional source"),
        input.iter().copied(),
        VmConfig::new(0, NonZeroU64::new(100).unwrap()),
    )
    .unwrap()
}

#[test]
fn cmp_peeks_unsigned_values_and_uses_the_selected_register() {
    for (a, b, expected) in [(7, 7, 0), (255, 0, 1), (0, 255, 2)] {
        let mut vm = machine("~> , ( } , ?= . ;", &[a, b]);
        assert_eq!(vm.run(30), RunOutcome::Halted);
        let state = vm.snapshot();
        assert_eq!(state.output, vec![expected]);
        assert_eq!(state.registers[0], a);
        assert_eq!(state.registers[1], expected);
        assert_eq!(state.threads[0].data_stack, vec![a]);
        assert_eq!(state.threads[0].register_pointer, 1);
        assert!(state
            .metrics
            .instruction_variety()
            .contains(&InstructionKind::Compare));
    }
}

#[test]
fn empty_cmp_preserves_register_and_repeat_recomputes_results() {
    let mut empty = machine("~> , ?= . ;", &[9]);
    assert_eq!(empty.run(20), RunOutcome::Halted);
    assert_eq!(empty.snapshot().output, vec![9]);
    assert_eq!(empty.snapshot().metrics.operation_count(), 4);
    let mut repeated = machine("~> , ( , ?=x3 . ;", &[2, 5]);
    assert_eq!(repeated.run(20), RunOutcome::Halted);
    assert_eq!(repeated.snapshot().output, vec![1]);
    assert_eq!(repeated.snapshot().threads[0].data_stack, vec![2]);
}

#[test]
fn false_prefix_skips_read_output_random_halt_and_suffix() {
    let mut vm = machine("~> ?1, ?1. ?1?? ?1; ?1+* ?0.3 ;", &[9]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.input, vec![9]);
    assert_eq!(state.output, vec![3]);
    assert_eq!(state.registers, [0; 10]);
    assert!(state.threads[0].instruction_stack.is_empty());
    assert_eq!(state.metrics.operation_count(), 8);
    assert_eq!(state.metrics.instruction_variety().len(), 3);
}

#[test]
fn guarded_repeat_stops_on_false_and_prefix_tests_before_pointer_change() {
    let mut vm = machine("~> + ?1+x3 ?2} ?0. ;", &[]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 2);
    assert_eq!(state.threads[0].register_pointer, 1);
    assert_eq!(state.output, vec![0]);
    assert_eq!(state.committed_ticks, 7);
}

#[test]
fn aftercall_does_not_recheck_a_changed_register() {
    let mut vm = machine(
        "@main\n~> ?0[0* % . ;\n@F0\n~> + ]\n@end F0\n@end main",
        &[],
    );
    assert_eq!(vm.run(30), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, vec![139]);
    assert_eq!(vm.snapshot().metrics.operation_count(), 8);
}

#[test]
fn false_calls_and_returns_preserve_control_flow() {
    let mut vm = machine(
        "@main\n~> ?1[0 ?1#0 [0 #0 ) . .3 ;\n@F0\n~> ?1] + ]\n@end F0\n@end main\n@C0\n~> ?2#] + . #]\n@end C0",
        &[],
    );
    assert_eq!(vm.run(40), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.registers[0], 1);
    assert_eq!(state.output, vec![1, 3]);
}

#[test]
fn cmp_readcode_suffix_exposes_only_primary_encoding() {
    let mut vm = machine("~> , ( , ?0?=* % . ;", &[7, 0]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let state = vm.snapshot();
    assert_eq!(state.output, vec![124]);
    assert_eq!(state.threads[0].data_stack, vec![7]);
}

#[test]
fn cleared_code_retains_prefix_and_true_suffix_behavior() {
    let mut vm = machine("~> , & ?0+= % . ;", &[32]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    let state = vm.snapshot();
    // Register 32 fails the prefix, so neither Add nor WriteCode executes.
    assert!(state.runtime_program.main.cells[3].primary.is_some());
    assert_eq!(
        state.runtime_program.main.cells[3].prefix,
        Some(ConditionPrefix::Zero)
    );
    assert_eq!(state.output, vec![32]);
    let mut cleared = machine("~> , & ! ?0+= ;", &[32]);
    assert_eq!(cleared.run(20), RunOutcome::Halted);
    let state = cleared.snapshot();
    assert!(state.runtime_program.main.cells[4].primary.is_none());
    assert_eq!(
        state.runtime_program.main.cells[4].prefix,
        Some(ConditionPrefix::Zero)
    );
    assert!(state.runtime_program.main.cells[4].attachment.is_some());
}

#[test]
fn fold_prefixes_execute_and_custom_cmp_uses_its_internal_stack() {
    let mut folded = machine("@main\n~> $0 ;\n@M0 ?1+ ?0.3 ^\n@end main", &[]);
    assert_eq!(folded.run(20), RunOutcome::Halted);
    assert_eq!(folded.snapshot().output, vec![3]);
    let mut custom = machine(
        "@main\n~> + ( #0 ) . ;\n@end main\n@C0\n~> + ?= . #]\n@end C0",
        &[],
    );
    assert_eq!(custom.run(30), RunOutcome::Halted);
    // Internal CMP sees its own empty stack and leaves its register at 1.
    assert_eq!(custom.snapshot().output, vec![2]);
    assert_eq!(custom.snapshot().threads[0].data_stack, vec![1]);
}

#[test]
fn concurrent_conditions_read_tick_start_and_skips_wrap() {
    let mut vm = machine("~> + ;\n~> ?0. ;", &[]);
    assert_eq!(vm.run(20), RunOutcome::Halted);
    assert_eq!(vm.snapshot().output, vec![0]);
    assert_eq!(vm.snapshot().registers[0], 1);
    let mut boundary = machine("~> ?1;", &[]);
    boundary.run(10);
    assert_eq!(boundary.status(), VmStatus::Running);
    assert_eq!(boundary.committed_ticks(), 10);
}

#[test]
fn obsolete_instruction_bytes_no_longer_decode() {
    for value in [63, 95, 97, 129, 153] {
        let mut vm = machine("~> , & % . ;", &[value]);
        assert_eq!(vm.run(20), RunOutcome::Halted);
        assert!(vm.snapshot().threads[0].instruction_stack.is_empty());
        assert_eq!(vm.snapshot().output, vec![value]);
    }
    for value in [124, 126] {
        let mut vm = machine("~> , & % . ;", &[value]);
        assert_eq!(vm.run(20), RunOutcome::Halted);
        assert_eq!(vm.snapshot().output, vec![value]);
    }
}

#[test]
fn cleared_cells_recheck_conditions_before_the_retained_suffix() {
    for (primary, expected_register, revisit_cost) in [("!", 0, 2), ("+", 1, 1)] {
        let source = format!("~> , & ! ?0{primary}= <");
        let mut vm = machine(&source, &[32]);
        for _ in 0..6 {
            vm.step();
        }
        let before = vm.snapshot();
        assert!(before.runtime_program.main.cells[4].primary.is_none());
        assert_eq!(before.registers[0], expected_register);
        vm.step();
        let after = vm.snapshot();
        assert_eq!(
            after.metrics.operation_count() - before.metrics.operation_count(),
            revisit_cost
        );
        assert_eq!(after.threads[0].position.x, 3);
        assert_eq!(
            after.runtime_program.main.cells[4].prefix,
            Some(ConditionPrefix::Zero)
        );
    }
}

#[test]
fn cmp_register_conflicts_roll_back_without_popping_stacks() {
    let mut vm = machine("~> ( ?= ;\n~> ( ?= ;", &[]);
    vm.run(20);
    let state = vm.snapshot();
    assert_eq!(state.status, VmStatus::Error);
    assert_eq!(state.committed_ticks, 2);
    assert_eq!(state.registers, [0; 10]);
    for thread in &state.threads {
        assert_eq!(thread.data_stack, vec![0]);
    }
}

#[test]
fn conditional_run_matches_repeated_steps() {
    let source = "~> , ( , ?= ?1+x3 ?2. ;";
    let mut bounded = machine(source, &[255, 0]);
    let mut stepped = machine(source, &[255, 0]);
    assert_eq!(bounded.run(30), RunOutcome::Halted);
    while stepped.status() == VmStatus::Running {
        stepped.step();
    }
    assert_eq!(bounded.snapshot(), stepped.snapshot());
}
