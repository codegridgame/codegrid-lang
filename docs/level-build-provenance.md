# Level Build Provenance v1

The local repeat command checks [the pinned toolchain inputs](../tools/level-build-toolchain-v1.json)
before building. A tool mismatch fails explicitly; it does not install or change
the user's tools. Updating these pins requires a fresh recorded host verification.

Level replay separates evaluator source identity from compiled artifact and
toolchain identity. `evaluator_build` is `codegrid-level-source-sha256:` followed
by a lowercase SHA-256 digest computed at build time. Native and WASM results
use the same source identity; it is not a mutable package-version placeholder.

The build script hashes root Cargo manifests/lock data and Rust source/Cargo
manifests for model, syntax, HIR, IR, compiler, VM, level-core, and level-api.
Inputs are sorted by slash-relative repository path. Each path and file content
is preceded by its byte length as a little-endian u64. Exact source bytes are
used, including line endings. The script is a build-host operation; evaluation
does not read files, environment variables, clocks, or process data.

`scripts/test-level-wasm.ps1` uses locked dependency resolution, explicit release
wasm32 builds, and explicit linear-memory maxima. After actual-host comparisons,
`scripts/record-level-build.mjs` writes `target/level-build-provenance.json` with
Rust compiler details, Cargo/wasm-bindgen/Node versions, native/WASM targets,
API/transport/format versions, both Cargo.lock digests, and browser/portable
artifact digests. Archive this report with the host result reports and the
exact source, level, and trusted profile used for replay.

The source ID identifies implementation inputs, not every possible compiler
flag or binary. Artifact digest and recorded toolchain/build configuration are
required when reproducing a specific artifact. Using the same source ID alone
does not certify a different compiler, engine, deployment, or Steam integration.
The repository records concrete toolchain inputs for each verified build rather
than silently claiming compatibility with an arbitrary installed toolchain.
