//! Actual portable Level ABI v1 execution against native CLI permitted results.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
use wasmtime::{
    Config, Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

struct Host {
    store: Store<StoreLimits>,
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    free: TypedFunc<(u32, u32), u32>,
    request: TypedFunc<(u32, u32), u64>,
}
impl Host {
    fn new(
        engine: &Engine,
        module: &Module,
        memory_limit: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut store = Store::new(
            engine,
            StoreLimitsBuilder::new()
                .memory_size(memory_limit)
                .instances(1)
                .memories(1)
                .tables(1)
                .build(),
        );
        store.limiter(|s| s);
        store.set_fuel(1_000_000_000)?;
        let instance = Instance::new(&mut store, module, &[])?;
        assert_eq!(
            instance
                .get_typed_func::<(), u32>(&mut store, "level_abi_version")?
                .call(&mut store, ())?,
            1
        );
        Ok(Self {
            memory: instance
                .get_memory(&mut store, "memory")
                .ok_or("Missing memory")?,
            alloc: instance.get_typed_func(&mut store, "level_alloc")?,
            free: instance.get_typed_func(&mut store, "level_dealloc")?,
            request: instance.get_typed_func(&mut store, "level_request")?,
            store,
        })
    }
    fn exchange(&mut self, value: Value) -> Result<Value, Box<dyn std::error::Error>> {
        self.store.set_fuel(1_000_000_000)?;
        let bytes = value.to_string().into_bytes();
        let length = u32::try_from(bytes.len())?;
        let pointer = self.alloc.call(&mut self.store, length)?;
        if pointer == 0 {
            return Err("Allocation failed".into());
        }
        self.memory
            .write(&mut self.store, pointer as usize, &bytes)?;
        let packed = self.request.call(&mut self.store, (pointer, length))?;
        assert_eq!(self.free.call(&mut self.store, (pointer, length))?, 1);
        if packed == 0 {
            return Err("ABI request failed".into());
        }
        let response_pointer = (packed >> 32) as u32;
        let response_length = packed as u32;
        if response_length > 8 * 1024 * 1024 {
            return Err("Unbounded response".into());
        }
        let mut response = vec![0; response_length as usize];
        self.memory
            .read(&self.store, response_pointer as usize, &mut response)?;
        assert_eq!(
            self.free
                .call(&mut self.store, (response_pointer, response_length))?,
            1
        );
        Ok(serde_json::from_slice(&response)?)
    }
    fn api(&mut self, mut value: Value) -> Result<Value, Box<dyn std::error::Error>> {
        value["api_version"] = json!(1);
        self.exchange(json!({"abi_version":1,"api_version":1,"operation":"request","request_json":value.to_string()}))
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let root = Path::new(
        args.get(1)
            .ok_or("Usage: level-parity <repository-root> <wasm-artifact>")?,
    );
    let wasm = args.get(2).ok_or("Missing WASM path")?;
    let memory_limit = std::env::var("CODEGRID_WASM_MAX_MEMORY_BYTES")
        .unwrap_or_else(|_| "67108864".into())
        .parse::<usize>()?;
    if memory_limit == 0 || memory_limit % 65536 != 0 {
        return Err("Memory ceiling must be a positive multiple of 65536 bytes".into());
    }
    let mut config = Config::new();
    config.consume_fuel(true);
    config.max_wasm_stack(2 * 1024 * 1024);
    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, wasm)?;
    assert_eq!(module.imports().count(), 0);
    let memory_type = module
        .exports()
        .find_map(|export| match export.ty() {
            wasmtime::ExternType::Memory(memory) if export.name() == "memory" => Some(memory),
            _ => None,
        })
        .ok_or("Missing exported memory type")?;
    if memory_type.maximum() != Some((memory_limit / 65536) as u64) {
        return Err("Artifact memory maximum differs from configured host ceiling".into());
    }
    let profile = root.join("fixtures/levels/profiles/local-v1.json");
    let profile_json = fs::read_to_string(&profile)?;
    let manifest_path = root.join("fixtures/levels/conformance-v1.json");
    let cases: Vec<Value> = if manifest_path.exists() {
        serde_json::from_str::<Value>(&fs::read_to_string(manifest_path)?)?["cases"]
            .as_array()
            .ok_or("Manifest cases required")?
            .clone()
    } else {
        vec![
            json!({"id":"echo","level":"echo.json","program":"echo.cg","mode":"Official","expected_status":"Passed"}),
        ]
    };
    let mut records = vec![];
    for case in cases {
        let level = root
            .join("fixtures/levels")
            .join(case["level"].as_str().ok_or("Level path required")?);
        let program = root
            .join("fixtures/levels")
            .join(case["program"].as_str().ok_or("Program path required")?);
        let mode = case["mode"].as_str().unwrap_or("Official");
        let boundary = case["boundary"].as_str().unwrap_or("Exit");
        let seed = case["seed"].as_str().unwrap_or("18446744073709551615");
        let custom_limit = case["custom_limit"].as_str().unwrap_or("1000");
        let native = Command::new(root.join(if cfg!(windows) {
            "target/debug/codegrid.exe"
        } else {
            "target/debug/codegrid"
        }))
        .arg("evaluate")
        .arg(&level)
        .arg(&program)
        .args([
            "--mode",
            &mode.to_ascii_lowercase(),
            "--boundary",
            &boundary.to_ascii_lowercase(),
            "--seed",
            seed,
            "--custom-limit",
            custom_limit,
            "--limits-file",
        ])
        .arg(&profile)
        .output()?;
        let native: Value = serde_json::from_slice(&native.stdout)?;
        let mut host = Host::new(&engine, &module, memory_limit)?;
        assert_eq!(host.exchange(json!({"abi_version":1,"api_version":1,"operation":"initialize","profile_json":profile_json}))?["status"],"ok");
        let level_response =
            host.api(json!({"operation":"load_level","level_json":fs::read_to_string(&level)?}))?;
        let response = if level_response["status"] != "ok" {
            level_response
        } else {
            let program_response = host.api(
                json!({"operation":"compile_program","source":fs::read_to_string(&program)?}),
            )?;
            if program_response["status"] != "ok" {
                program_response
            } else {
                let started=host.api(json!({"operation":"start_evaluation","level":level_response["handle"],"program":program_response["handle"],"mode":mode,"boundary_mode":boundary,"shuffle_seed":seed,"custom_execution_limit":custom_limit}))?;
                if started["status"] != "ok" {
                    started
                } else {
                    loop {
                        let result=host.api(json!({"operation":"advance_evaluation","evaluation":started["handle"],"work_budget":"1000000"}))?;
                        if result["status"] != "pending" {
                            break result;
                        }
                    }
                }
            }
        };
        let mut portable = response;
        portable.as_object_mut().unwrap().remove("abi_version");
        assert_eq!(portable, native, "Fixture {}", case["id"]);
        records.push(json!({"id":case["id"],"response":portable}));
    }
    let mut probe = Host::new(&engine, &module, memory_limit)?;
    assert_eq!(probe.exchange(json!({"abi_version":1,"api_version":1,"operation":"initialize","profile_json":profile_json}))?["status"],"ok");
    assert_eq!(
        probe.api(json!({"operation":"compile_program","source":"INVALID"}))?["status"],
        "source_rejected"
    );
    assert_eq!(
        probe.api(json!({"operation":"load_level","level_json":"{}"}))?["status"],
        "level_rejected"
    );
    let environment = serde_json::json!({"format_version":1,"level_id":"unsupported","level_version":1,"evaluation_type":"Environment","program_rules":{},"constraints":{},"scoring":{},"evaluation":{"scene_type":"Elevator"}});
    assert_eq!(
        probe.api(json!({"operation":"load_level","level_json":environment.to_string()}))?["error"]
            ["code"],
        "level.unsupported_scene_type"
    );
    assert_eq!(
        probe.exchange(
            json!({"abi_version":1,"api_version":1,"operation":"request","request_json":"{"})
        )?["error"]["code"],
        "level_api.invalid_request"
    );
    assert_eq!(
        probe.api(json!({"operation":"evaluation_result","evaluation":"0"}))?["error"]["code"],
        "level_api.invalid_handle"
    );
    let mut tiny_profile: Value = serde_json::from_str(&profile_json)?;
    tiny_profile["max_response_bytes"] = json!("512");
    let mut small = Host::new(&engine, &module, memory_limit)?;
    assert_eq!(small.exchange(json!({"abi_version":1,"api_version":1,"operation":"initialize","profile_json":tiny_profile.to_string()}))?["status"],"ok");
    assert_eq!(small.api(json!({"operation":"compile_program","source":format!("~> {}",vec!["INVALID";50].join(" "))}))?["error"]["code"],"level_api.response_too_large");
    probe.store.set_fuel(1)?;
    assert!(
        probe.alloc.call(&mut probe.store, 1).is_err(),
        "Fuel exhaustion must trap rather than certify a result"
    );
    let artifact_digest = format!("{:x}", Sha256::digest(fs::read(wasm)?));
    let report = json!({"runtime":"Wasmtime 49.0.1","surface":"Local portable backend/desktop harness; not production integration","memory_limit_bytes":memory_limit.to_string(),"artifact_sha256":artifact_digest,"profile":serde_json::from_str::<Value>(&profile_json)?,"negative_cases":["source-rejection","level-rejection","unsupported-scene","malformed-request","invalid-handle","response-ceiling","fuel-trap"],"cases":records});
    fs::write(
        root.join("target/level-wasmtime-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Wasmtime level parity passed: {} complete CLI responses",
        records.len()
    );
    Ok(())
}
