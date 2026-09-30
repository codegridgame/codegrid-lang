//! Stable evaluator source identity shared by native and WASM builds.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn files(directory: &Path, collected: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", directory.display());
    for entry in fs::read_dir(directory).expect("Read evaluator source directory") {
        let path = entry.expect("Read evaluator source entry").path();
        if path.is_dir() {
            files(&path, collected);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            || path.file_name().is_some_and(|name| name == "Cargo.toml")
        {
            collected.push(path);
        }
    }
}
fn main() {
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"));
    let root = manifest
        .parent()
        .and_then(Path::parent)
        .expect("Workspace layout");
    let mut collected = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    for name in [
        "codegrid-model",
        "codegrid-syntax",
        "codegrid-hir",
        "codegrid-ir",
        "codegrid-compiler",
        "codegrid-vm",
        "codegrid-level-core",
        "codegrid-level-api",
    ] {
        files(&root.join("crates").join(name), &mut collected);
    }
    collected.sort_by_key(|path| {
        path.strip_prefix(root)
            .expect("Workspace source")
            .to_string_lossy()
            .replace('\\', "/")
    });
    let mut digest = Sha256::new();
    for path in collected {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path
            .strip_prefix(root)
            .expect("Workspace source")
            .to_string_lossy()
            .replace('\\', "/");
        let contents = fs::read(&path).expect("Read evaluator source identity input");
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update((contents.len() as u64).to_le_bytes());
        digest.update(&contents);
    }
    println!(
        "cargo:rustc-env=CODEGRID_LEVEL_BUILD_ID=codegrid-level-source-sha256:{:x}",
        digest.finalize()
    );
}
