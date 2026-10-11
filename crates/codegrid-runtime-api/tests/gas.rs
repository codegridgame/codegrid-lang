use codegrid_runtime_api::{
    ApiError, CompileOutcome, CompileRequest, CreateInstanceRequest, HostLimits, InstanceHandle,
    RunRequest, RunStatus, RuntimeApi, RuntimeConfiguration, SnapshotRequest, StepRequest,
    VmStatus, YieldReason, RUNTIME_API_VERSION,
};
use std::num::NonZeroU64;

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap()
}
fn api(work: u64, gas: u64) -> RuntimeApi {
    RuntimeApi::new(
        HostLimits::new(16384, 8, 8, 1024, 8, nz(100), nz(10000), nz(work))
            .with_max_gas_per_instance(nz(gas)),
    )
}
fn create(api: &mut RuntimeApi, source: &str, limit: u64) -> Result<InstanceHandle, ApiError> {
    let program = match api
        .compile(CompileRequest {
            api_version: RUNTIME_API_VERSION,
            source: source.into(),
        })
        .unwrap()
        .outcome
    {
        CompileOutcome::Compiled { program } => program,
        other => panic!("valid Gas test program: {other:?}"),
    };
    api.create_instance(CreateInstanceRequest {
        api_version: RUNTIME_API_VERSION,
        program,
        input: vec![9],
        initial_memory: vec![],
        configuration: RuntimeConfiguration {
            seed: 42,
            custom_execution_limit: 100,
            gas_hard_limit: limit,
        },
    })
    .map(|result| result.instance)
}
fn snapshot(api: &RuntimeApi, instance: InstanceHandle) -> codegrid_runtime_api::RuntimeSnapshot {
    api.snapshot(SnapshotRequest {
        api_version: RUNTIME_API_VERSION,
        instance,
    })
    .unwrap()
    .snapshot
}
#[test]
fn gas_request_must_be_positive_and_within_trusted_ceiling() {
    let mut runtime = api(100, 10);
    for limit in [0, 11, u64::MAX] {
        assert!(matches!(
            create(&mut runtime, "~> ;", limit),
            Err(ApiError::InvalidConfiguration {
                field: "gas_hard_limit"
            })
        ));
    }
    assert!(create(&mut runtime, "~> ;", 10).is_ok());
}
#[test]
fn explicit_u64_max_is_exact_when_trusted_profile_allows_it() {
    let mut runtime = api(100, u64::MAX);
    let instance = create(&mut runtime, "~> , . ;", u64::MAX).unwrap();
    let result = runtime
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_ticks: 20,
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Halted);
    assert_eq!(result.snapshot.output, vec![9]);
    assert_eq!(result.snapshot.metrics.gas_used(), 10);
    assert_eq!(result.snapshot.metrics.execution_gas(), 10);
    assert_eq!(result.snapshot.metrics.gas_schedule_version(), 1);
}
#[test]
fn gas_limit_rolls_back_effects_and_terminal_retry_never_rebills() {
    let mut runtime = api(100, 100);
    let instance = create(&mut runtime, "~> + . ;", 3).unwrap();
    let result = runtime
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_ticks: 20,
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Error);
    assert!(result.snapshot.output.is_empty());
    assert_eq!(result.snapshot.registers[0], 1);
    assert_eq!(result.snapshot.metrics.gas_used(), 8);
    assert_eq!(result.snapshot.errors[0].code(), "GasLimitExceeded");
    let before = result.snapshot;
    let again = runtime
        .step(StepRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
        })
        .unwrap();
    assert_eq!(again.result.status, VmStatus::Error);
    assert_eq!(snapshot(&runtime, instance), before);
}
#[test]
fn custom_dispatch_is_a_structured_error_while_definition_survives_compilation() {
    let mut runtime = api(100, 100);
    let instance = create(
        &mut runtime,
        "@main\n~> #0 ;\n@end main\n@C0\n~> + . #]\n@end C0",
        100,
    )
    .unwrap();
    let result = runtime
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_ticks: 20,
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Error);
    assert_eq!(result.snapshot.errors[0].code(), "CustomDisabled");
    assert!(result.snapshot.output.is_empty());
    assert_eq!(result.snapshot.metrics.gas_used(), 0);
}
#[test]
fn step_and_small_work_run_have_identical_gas_and_snapshots() {
    let source = "~> , ( + ) . ;";
    let mut stepped = api(100, 1000);
    let step_instance = create(&mut stepped, source, 1000).unwrap();
    for _ in 0..30 {
        if stepped
            .step(StepRequest {
                api_version: RUNTIME_API_VERSION,
                instance: step_instance,
            })
            .unwrap()
            .result
            .status
            != VmStatus::Running
        {
            break;
        }
    }
    let expected = snapshot(&stepped, step_instance);
    assert_eq!(expected.status, VmStatus::Halted);
    let mut sliced = api(2, 1000);
    let run_instance = create(&mut sliced, source, 1000).unwrap();
    let mut yields = 0;
    for _ in 0..30 {
        let result = sliced
            .run(RunRequest {
                api_version: RUNTIME_API_VERSION,
                instance: run_instance,
                max_ticks: 20,
            })
            .unwrap();
        match result.status {
            RunStatus::Halted => break,
            RunStatus::Yielded {
                reason: YieldReason::WorkUnitBudgetExhausted,
            } => yields += 1,
            other => panic!("unexpected bounded-run outcome: {other:?}"),
        }
    }
    assert!(yields > 0);
    assert_eq!(snapshot(&sliced, run_instance), expected);
}
#[test]
fn work_interrupted_tick_leaves_gas_and_vm_state_unchanged() {
    let mut runtime = api(1, 1000);
    let instance = create(&mut runtime, "@main\n@size 2x1\n~> ~<\n@end main", 1000).unwrap();
    let before = snapshot(&runtime, instance);
    assert!(matches!(
        runtime.step(StepRequest {
            api_version: RUNTIME_API_VERSION,
            instance
        }),
        Err(ApiError::WorkUnitBudgetExceeded { .. })
    ));
    assert_eq!(snapshot(&runtime, instance), before);
    assert!(matches!(
        runtime.step(StepRequest {
            api_version: RUNTIME_API_VERSION,
            instance
        }),
        Err(ApiError::WorkUnitBudgetExceeded { .. })
    ));
    assert_eq!(snapshot(&runtime, instance), before);
}
