use codegrid_runtime_api::{
    ApiError, CompileOutcome, CompileRequest, CreateInstanceRequest, HostLimits, InstanceHandle,
    RunRequest, RunStatus, RuntimeApi, RuntimeConfiguration, SnapshotRequest, StepRequest,
    VmStatus, YieldReason, RUNTIME_API_VERSION,
};
use std::num::NonZeroU64;

fn nz(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).expect("host work-unit limits must be positive")
}

fn api(work_units: u64) -> RuntimeApi {
    RuntimeApi::new(HostLimits::new(
        16_384,
        8,
        8,
        1024,
        8,
        nz(100),
        nz(10_000),
        nz(work_units),
    ))
}

fn create(api: &mut RuntimeApi, source: &str) -> Result<InstanceHandle, ApiError> {
    let program = match api
        .compile(CompileRequest {
            api_version: RUNTIME_API_VERSION,
            source: source.into(),
        })
        .expect("source fits host limits")
        .outcome
    {
        CompileOutcome::Compiled { program } => program,
        other => panic!("valid runtime test program: {other:?}"),
    };
    api.create_instance(CreateInstanceRequest {
        api_version: RUNTIME_API_VERSION,
        program,
        input: vec![9],
        initial_memory: vec![],
        configuration: RuntimeConfiguration { seed: 42 },
    })
    .map(|result| result.instance)
}

fn snapshot(api: &RuntimeApi, instance: InstanceHandle) -> codegrid_runtime_api::RuntimeSnapshot {
    api.snapshot(SnapshotRequest {
        api_version: RUNTIME_API_VERSION,
        instance,
    })
    .expect("instance is live")
    .snapshot
}

#[test]
fn bounded_run_matches_repeated_steps_when_work_units_force_yields() {
    let source = "~> , ( + ) . ;";
    let mut stepped = api(100);
    let step_instance = create(&mut stepped, source).unwrap();
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

    let mut sliced = api(2);
    let run_instance = create(&mut sliced, source).unwrap();
    let mut yields = 0;
    for _ in 0..30 {
        let result = sliced
            .run(RunRequest {
                api_version: RUNTIME_API_VERSION,
                instance: run_instance,
                max_steps: 20,
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
fn work_interrupted_step_leaves_vm_state_unchanged() {
    let mut runtime = api(1);
    let instance = create(&mut runtime, "@main\n@size 2x1\n~> ~<\n@end main").unwrap();
    let before = snapshot(&runtime, instance);
    for _ in 0..2 {
        assert!(matches!(
            runtime.step(StepRequest {
                api_version: RUNTIME_API_VERSION,
                instance
            }),
            Err(ApiError::WorkUnitBudgetExceeded { .. })
        ));
        assert_eq!(snapshot(&runtime, instance), before);
    }
}

#[test]
fn custom_dispatch_is_supported_and_uses_the_generic_work_limit() {
    let source = "@main\n~> #0 ;\n@end main\n@C0\n~> + #]\n@end C0";
    let mut limited_runtime = api(2);
    let limited_instance = create(&mut limited_runtime, source).unwrap();
    let limited = limited_runtime
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance: limited_instance,
            max_steps: 10,
        })
        .unwrap();
    assert!(matches!(
        limited.status,
        RunStatus::Yielded {
            reason: YieldReason::WorkUnitBudgetExhausted
        }
    ));
    assert_eq!(limited.snapshot.status, VmStatus::Running);
    assert_eq!(limited.snapshot.committed_steps, 1);
    assert!(limited.snapshot.errors.is_empty());

    let mut runtime = api(100);
    let instance = create(&mut runtime, source).unwrap();
    let result = runtime
        .run(RunRequest {
            api_version: RUNTIME_API_VERSION,
            instance,
            max_steps: 10,
        })
        .unwrap();
    assert_eq!(result.status, RunStatus::Halted);
    assert!(result.snapshot.errors.is_empty());
    let result = snapshot(&runtime, instance);
    assert!(result.errors.is_empty());
}
