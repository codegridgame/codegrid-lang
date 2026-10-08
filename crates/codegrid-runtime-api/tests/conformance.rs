use std::num::NonZeroU64;

use codegrid_runtime_api::{
    ApiError, BoundaryMode, CheckRequest, CompileOutcome, CompileRequest, CreateInstanceRequest,
    HostLimits, MemoryAddress, MemoryEntry, ProgramHandle, ProgramViewRequest,
    ReleaseInstanceRequest, ReleaseProgramRequest, RunRequest, RunStatus, RuntimeApi,
    RuntimeConfiguration, SnapshotRequest, StepRequest, VmStatus, YieldReason, RUNTIME_API_VERSION,
};

fn limits(
    max_source_bytes: usize,
    max_programs: usize,
    max_instances: usize,
    max_input_bytes: usize,
    max_initial_memory_entries: usize,
    max_run_ticks: u64,
    max_total_ticks: u64,
    max_work_units: u64,
) -> HostLimits {
    HostLimits::new(
        max_source_bytes,
        max_programs,
        max_instances,
        max_input_bytes,
        max_initial_memory_entries,
        NonZeroU64::new(max_run_ticks).expect("positive run-tick limit"),
        NonZeroU64::new(max_total_ticks).expect("positive instance-tick limit"),
        NonZeroU64::new(max_work_units).expect("positive work limit"),
    )
}

fn runtime() -> RuntimeApi {
    RuntimeApi::new(limits(16 * 1024, 8, 8, 1024, 8, 100, 10_000, 100_000))
}

fn compile_source(runtime: &mut RuntimeApi, source: &str) -> ProgramHandle {
    let response = runtime
        .compile(CompileRequest {
            api_version: RUNTIME_API_VERSION,
            source: source.to_owned(),
        })
        .expect("source fits host limits");
    match response.outcome {
        CompileOutcome::Compiled { program } => program,
        CompileOutcome::Diagnostics { items } => panic!("expected valid source: {items:?}"),
    }
}

fn configuration(seed: u64, custom_execution_limit: u64) -> RuntimeConfiguration {
    RuntimeConfiguration {
        boundary_mode: BoundaryMode::Wrap,
        seed,
        custom_execution_limit,
    }
}

fn create_instance(
    runtime: &mut RuntimeApi,
    program: ProgramHandle,
    input: Vec<u8>,
    initial_memory: Vec<MemoryEntry>,
    configuration: RuntimeConfiguration,
) -> codegrid_runtime_api::InstanceHandle {
    runtime
        .create_instance(CreateInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            program,
            input,
            initial_memory,
            configuration,
        })
        .expect("valid v3 instance request")
        .instance
}

#[test]
fn v3_compile_and_program_view_use_verified_handle_lifecycle() {
    let mut api = runtime();
    assert_eq!(api.api_version(), 3);
    assert!(api
        .check(CheckRequest {
            api_version: RUNTIME_API_VERSION,
            source: "@main\n~> #0 ;\n@end main\n@C0\n~> #]\n@end C0\n".to_owned(),
        })
        .unwrap()
        .diagnostics
        .is_empty());

    let program = compile_source(&mut api, "@main\n~> #0 ;\n@end main\n@C0\n~> #]\n@end C0\n");
    let borrowed = api
        .program_view_ref(ProgramViewRequest {
            api_version: RUNTIME_API_VERSION,
            program,
        })
        .expect("live program can be inspected through a borrowed view");
    assert_eq!(borrowed.view.ir_format_version(), 3);
    assert_eq!(borrowed.view.program().customs.len(), 1);
    let owned = borrowed.into_owned();
    assert_eq!(owned.api_version, RUNTIME_API_VERSION);
    assert!(owned.view.customs.contains_key(&0));
    assert_eq!(
        api.program_view(ProgramViewRequest {
            api_version: 1,
            program,
        }),
        Err(ApiError::UnsupportedVersion {
            received: 1,
            supported: RUNTIME_API_VERSION,
        })
    );

    api.release_program(ReleaseProgramRequest {
        api_version: RUNTIME_API_VERSION,
        program,
    })
    .unwrap();
    assert_eq!(
        api.program_view(ProgramViewRequest {
            api_version: RUNTIME_API_VERSION,
            program,
        }),
        Err(ApiError::UnknownProgramHandle { handle: program })
    );
}

#[test]
fn instance_creation_preserves_full_configuration_and_sparse_bigint_memory() {
    let mut api = runtime();
    let program = compile_source(&mut api, "@main\n@size 1x1\n~>\n@end main\n");
    let large_address: MemoryAddress = (MemoryAddress::from(1u8) << 192usize) - 7;
    let negative_address = MemoryAddress::from(-19);
    let instance = create_instance(
        &mut api,
        program,
        vec![4, 5],
        vec![
            MemoryEntry {
                address: large_address.clone(),
                value: 231,
            },
            MemoryEntry {
                address: negative_address.clone(),
                value: 0,
            },
        ],
        configuration(u64::MAX, u64::MAX),
    );

    let snapshot = api
        .snapshot(SnapshotRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        })
        .unwrap()
        .snapshot;
    assert_eq!(snapshot.status, VmStatus::Running);
    assert_eq!(snapshot.remaining_input, vec![4, 5]);
    assert_eq!(snapshot.memory.get(&large_address), Some(&231));
    assert!(!snapshot.memory.contains_key(&negative_address));
    assert_eq!(snapshot.runtime_program.main.width, 1);
    assert_eq!(snapshot.runtime_program.main.height, 1);
    assert_eq!(snapshot.threads.len(), 1);
    assert_eq!(snapshot.threads[0].id, 0);
    assert_eq!(snapshot.metrics.global_tick(), 0);
}

#[test]
fn initial_memory_and_configuration_rejections_do_not_allocate_instances() {
    let mut api = RuntimeApi::new(limits(1024, 2, 1, 4, 1, 8, 32, 100));
    let program = compile_source(&mut api, "@main\n@size 1x1\n~>\n@end main\n");
    let duplicate_address = MemoryAddress::from(-3);
    let request = |initial_memory: Vec<MemoryEntry>, configuration| CreateInstanceRequest {
        api_version: RUNTIME_API_VERSION,
        program,
        input: Vec::new(),
        initial_memory,
        configuration,
    };

    assert_eq!(
        api.create_instance(request(
            vec![
                MemoryEntry {
                    address: duplicate_address.clone(),
                    value: 1,
                },
                MemoryEntry {
                    address: duplicate_address.clone(),
                    value: 0,
                },
            ],
            configuration(0, 1),
        )),
        Err(ApiError::InitialMemoryLimitExceeded {
            received_entries: 2,
            maximum: 1,
        })
    );
    assert_eq!(
        api.create_instance(request(
            vec![MemoryEntry {
                address: duplicate_address.clone(),
                value: 1,
            }],
            configuration(0, 0),
        )),
        Err(ApiError::InvalidConfiguration {
            field: "custom_execution_limit",
        })
    );

    // A duplicate remains invalid even when one entry is zero-valued.
    let mut api = RuntimeApi::new(limits(1024, 2, 1, 4, 2, 8, 32, 100));
    let program = compile_source(&mut api, "@main\n@size 1x1\n~>\n@end main\n");
    assert_eq!(
        api.create_instance(CreateInstanceRequest {
            api_version: RUNTIME_API_VERSION,
            program,
            input: Vec::new(),
            initial_memory: vec![
                MemoryEntry {
                    address: duplicate_address.clone(),
                    value: 0,
                },
                MemoryEntry {
                    address: duplicate_address.clone(),
                    value: 8,
                },
            ],
            configuration: configuration(0, 1),
        }),
        Err(ApiError::DuplicateInitialMemoryAddress {
            address: duplicate_address,
        })
    );
    let instance = create_instance(
        &mut api,
        program,
        Vec::new(),
        Vec::new(),
        configuration(0, 1),
    );
    assert_eq!(
        instance.get(),
        1,
        "rejected requests must not consume handles"
    );
}

#[test]
fn run_deltas_match_the_ordered_results_of_repeated_step_calls() {
    let source = "~> , . ;\n";
    let mut run_api = runtime();
    let run_program = compile_source(&mut run_api, source);
    let run_instance = create_instance(
        &mut run_api,
        run_program,
        vec![93],
        Vec::new(),
        configuration(77, 100),
    );
    let run_result = run_api
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance: run_instance,
            max_ticks: 16,
        })
        .expect("bounded run fits request and instance ceilings");

    let mut step_api = runtime();
    let step_program = compile_source(&mut step_api, source);
    let step_instance = create_instance(
        &mut step_api,
        step_program,
        vec![93],
        Vec::new(),
        configuration(77, 100),
    );
    let mut events = Vec::new();
    let mut output = Vec::new();
    let mut status = VmStatus::Running;
    while status == VmStatus::Running {
        let result = step_api
            .step(StepRequest {
                api_version: RUNTIME_API_VERSION,
                instance: step_instance,
            })
            .expect("one step is allowed");
        status = result.result.status;
        events.extend(result.result.events);
        output.extend(result.result.newly_emitted_output);
    }
    let final_snapshot = step_api
        .snapshot(SnapshotRequest {
            api_version: RUNTIME_API_VERSION,
            instance: step_instance,
        })
        .unwrap()
        .snapshot;

    assert_eq!(run_result.status, RunStatus::Halted);
    assert_eq!(run_result.newly_emitted_output, vec![93]);
    assert_eq!(run_result.events, events);
    assert_eq!(run_result.newly_emitted_output, output);
    assert_eq!(run_result.snapshot, final_snapshot);
    assert!(!run_result.events.is_empty());
    assert_eq!(run_result.snapshot.output, vec![93]);
    assert_eq!(run_result.snapshot.status, VmStatus::Halted);
}

#[test]
fn run_slice_yields_with_full_borrowed_snapshot_and_release_is_instance_local() {
    let mut api = RuntimeApi::new(limits(1024, 2, 2, 8, 0, 2, 8, 100));
    let program = compile_source(&mut api, "@main\n@size 1x1\n~>\n@end main\n");
    let first = create_instance(
        &mut api,
        program,
        Vec::new(),
        Vec::new(),
        configuration(5, 9),
    );
    let second = create_instance(&mut api, program, vec![7], Vec::new(), configuration(8, 9));

    let response = api
        .run_view(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance: first,
            max_ticks: 2,
        })
        .expect("bounded run should return a borrowed Full snapshot");
    assert_eq!(
        response.status,
        RunStatus::Yielded {
            reason: YieldReason::TickSliceExhausted,
        }
    );
    assert_eq!(response.snapshot.status(), VmStatus::Running);
    assert_eq!(response.snapshot.committed_ticks(), 2);
    assert_eq!(response.snapshot.threads().count(), 1);
    assert_eq!(response.snapshot.metrics().global_tick(), 2);
    let response = response.into_owned();
    assert_eq!(response.snapshot.committed_ticks, 2);

    api.release_instance(ReleaseInstanceRequest {
        api_version: RUNTIME_API_VERSION,
        instance: first,
    })
    .unwrap();
    assert_eq!(
        api.snapshot(SnapshotRequest {
            api_version: RUNTIME_API_VERSION,
            instance: first,
        }),
        Err(ApiError::UnknownInstanceHandle { handle: first })
    );
    assert_eq!(
        api.snapshot(SnapshotRequest {
            api_version: RUNTIME_API_VERSION,
            instance: second,
        })
        .unwrap()
        .snapshot
        .remaining_input,
        vec![7],
        "releasing one instance must leave other instances intact"
    );
}
