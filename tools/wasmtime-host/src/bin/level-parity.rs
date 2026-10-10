//! Actual portable Scene Level ABI 2 execution against native CLI permitted results.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
use wasmtime::{
    Config, Engine, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

struct Host {
    version: u32,
    store: Store<StoreLimits>,
    memory: Memory,
    alloc: TypedFunc<u32, u32>,
    free: TypedFunc<(u32, u32), u32>,
    request: TypedFunc<(u32, u32), u64>,
}
impl Host {
    fn new_versioned(
        engine: &Engine,
        module: &Module,
        memory_limit: usize,
        version: u32,
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
            2
        );
        Ok(Self {
            version,
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
        value["api_version"] = json!(self.version);
        self.exchange(json!({"abi_version":self.version,"api_version":self.version,"operation":"request","request_json":value.to_string()}))
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let root = Path::new(
        args.get(1)
            .ok_or("Usage: level-parity <repository-root> <wasm-artifact>")?,
    );
    let wasm = args.get(2).ok_or("Missing WASM path")?;
    let version = 2;
    let fixtures = root.join("fixtures/scene-v2");
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
    let profile = root.join("examples/scene-host-v2/profile-local-v2.json");
    let profile_json = fs::read_to_string(&profile)?;
    let manifest_path = fixtures.join("conformance-v2.json");
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
        let level = fixtures.join(case["level"].as_str().ok_or("Level path required")?);
        let program = fixtures.join(case["program"].as_str().ok_or("Program path required")?);
        let mode = case["mode"].as_str().unwrap_or("Official");
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
            "--seed",
            seed,
            "--custom-limit",
            custom_limit,
            "--limits-file",
        ])
        .arg(&profile)
        .args(["--api-version", &version.to_string()])
        .output()?;
        let native: Value = serde_json::from_slice(&native.stdout)?;
        let mut host = Host::new_versioned(&engine, &module, memory_limit, version)?;
        assert_eq!(host.exchange(json!({"abi_version":version,"api_version":version,"operation":"initialize","profile_json":profile_json}))?["status"],"ok");
        let level_response =
            host.api(json!({"operation":"load_level","level_json":fs::read_to_string(&level)?}))?;
        let mut evaluation = Value::Null;
        let response = if level_response["status"] != "ok" {
            level_response
        } else {
            let program_response = host.api(
                json!({"operation":"compile_program","source":fs::read_to_string(&program)?}),
            )?;
            if program_response["status"] != "ok" {
                program_response
            } else {
                let started=host.api(json!({"operation":"start_evaluation","level":level_response["handle"],"program":program_response["handle"],"mode":mode,"shuffle_seed":seed,"custom_execution_limit":custom_limit}))?;
                evaluation = started["handle"].clone();
                if started["status"] != "ok" {
                    started
                } else {
                    loop {
                        let result=host.api(json!({"operation":"advance_evaluation","evaluation":started["handle"],"work_budget":case["work_budget"].as_str().unwrap_or("1000000")}))?;
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
        {
            assert_eq!(portable["result"]["status"], case["expected_status"]);
            let mut events = Vec::new();
            if mode == "Debug" {
                let mut cursor = json!("0");
                for _ in 0..100 {
                    let page = host.api(json!({"operation":"scene_feedback","evaluation_handle":evaluation,"after_sequence":cursor,"max_events":"1024"}))?;
                    assert_eq!(page["status"], "ok");
                    events.extend(
                        page["events"]
                            .as_array()
                            .ok_or("Missing events")?
                            .iter()
                            .cloned(),
                    );
                    cursor = page["next_sequence"].clone();
                    if page["has_more"] == false {
                        break;
                    }
                }
            }
            records.push(json!({"id":case["id"],"result":portable["result"],"events":events}));
        }
    }
    {
        let mut fuel_probe = Host::new_versioned(&engine, &module, memory_limit, 2)?;
        fuel_probe.store.set_fuel(1)?;
        assert!(fuel_probe.alloc.call(&mut fuel_probe.store, 1).is_err());
        let report = json!({"runtime":"Wasmtime 49.0.1","memory_limit_bytes":memory_limit.to_string(),"artifact_sha256":format!("{:x}",Sha256::digest(fs::read(wasm)?)),"results":records});
        fs::write(
            root.join("target/scene-wasmtime-report.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!(
            "Wasmtime API-2 scene parity passed: {} complete CLI results and Debug traces",
            records.len()
        );
        return Ok(());
    }
}
