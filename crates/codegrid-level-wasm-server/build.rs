use std::env;

const WASM_PAGE_BYTES: u64 = 65_536;
const WASM32_MAX_MEMORY_BYTES: u64 = 4_294_967_296;

fn main() {
    println!("cargo:rerun-if-env-changed=CODEGRID_WASM_MAX_MEMORY_BYTES");

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }

    let configured = env::var("CODEGRID_WASM_MAX_MEMORY_BYTES").unwrap_or_else(|_| {
        panic!(
            "wasm32 builds require CODEGRID_WASM_MAX_MEMORY_BYTES; choose an explicit linear-memory ceiling"
        )
    });
    let maximum = configured.parse::<u64>().unwrap_or_else(|_| {
        panic!("CODEGRID_WASM_MAX_MEMORY_BYTES must be an unsigned decimal byte count")
    });
    if maximum == 0 || maximum % WASM_PAGE_BYTES != 0 || maximum > WASM32_MAX_MEMORY_BYTES {
        panic!(
            "CODEGRID_WASM_MAX_MEMORY_BYTES must be a positive multiple of 65536 and no greater than 4294967296"
        );
    }

    println!("cargo:rustc-link-arg=--max-memory={maximum}");
}
