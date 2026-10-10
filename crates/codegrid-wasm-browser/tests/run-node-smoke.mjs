import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, resolve } from "node:path";
import { runInNewContext } from "node:vm";
import { fileURLToPath } from "node:url";
import { runFullSmoke, RUNTIME_API_VERSION } from "./full-smoke.mjs";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(scriptDirectory, "../../..");
const targetDirectory = resolve(workspaceRoot, process.env.CARGO_TARGET_DIR ?? "target");
const generatedBindings = resolve(targetDirectory, "browser-node-bindings");
const bindingsPath = resolve(generatedBindings, "codegrid_wasm_browser.js");
const wasmPath = resolve(generatedBindings, "codegrid_wasm_browser_bg.wasm");
const suitePath = resolve(workspaceRoot, "tests/fixtures/conformance-v1.json");
const baselinePath = resolve(targetDirectory, "wasmtime-host/native-cli-full-baseline-v1.json");
const require = createRequire(import.meta.url);

for (const path of [bindingsPath, wasmPath, suitePath, baselinePath]) {
  try {
    readFileSync(path);
  } catch {
    throw new Error(`Missing Node WASM smoke input: ${path}`);
  }
}

const maximumBytes = configuredWasmMemoryLimit();
assert.equal(readDefinedWasmMemoryLimit(readFileSync(wasmPath)), maximumBytes);
const { BrowserRuntime } = require(bindingsPath);
const suite = JSON.parse(readFileSync(suitePath, "utf8"));
const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
const fixtureCount = runFullSmoke(
  BrowserRuntime,
  suite,
  baseline,
  (condition, message) => assert.ok(condition, message),
  (actual, expected, message) => assert.deepEqual(actual, expected, message),
);

const runtime = createRuntime(BrowserRuntime);
const compiled = parseJson(runtime.compile("~> , . ;\n"), "compile cross-realm fixture");
assert.equal(compiled.outcome.kind, "compiled");
const crossRealmInput = runInNewContext("new Uint8Array([97])");
const created = parseJson(
  runtime.create_instance(
    compiled.outcome.program,
    crossRealmInput,
    "[]",
    JSON.stringify({ seed: "0", custom_execution_limit: "10" }),
  ),
  "create cross-realm instance",
);
assert.equal(created.error, undefined, "cross-realm Uint8Array input must be accepted");
const result = parseJson(runtime.run(created.instance, "10"), "run cross-realm instance");
assert.deepEqual(result.snapshot.output, [97]);
assert.equal(parseJson(runtime.release_instance(created.instance), "release cross-realm instance").api_version, RUNTIME_API_VERSION);
assert.equal(parseJson(runtime.release_program(compiled.outcome.program), "release cross-realm program").api_version, RUNTIME_API_VERSION);

console.log(`PASS: Node WebAssembly Runtime API v${RUNTIME_API_VERSION}, ${fixtureCount} Full conformance fixtures, exact wide integers, lifecycle and quotas; configured linear-memory maximum ${maximumBytes} bytes.`);

function createRuntime(Runtime) {
  return new Runtime(1_048_576, 128, 128, 1_048_576, 65_536, 1_048_576, 1_048_576, 1_048_576, "10000", "10000", "1000000");
}

function parseJson(value, operation) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`${operation} returned invalid JSON: ${error}`);
  }
}

function configuredWasmMemoryLimit() {
  const value = process.env.CODEGRID_WASM_MAX_MEMORY_BYTES;
  if (!value || !/^\d+$/.test(value)) {
    throw new Error("Set CODEGRID_WASM_MAX_MEMORY_BYTES to verify the generated WASM memory ceiling.");
  }
  const maximum = BigInt(value);
  if (maximum <= 0n || maximum > 4_294_967_296n || maximum % 65_536n !== 0n) {
    throw new Error("CODEGRID_WASM_MAX_MEMORY_BYTES must be a positive 65536-byte multiple no greater than 4294967296.");
  }
  return maximum;
}

function readDefinedWasmMemoryLimit(bytes) {
  assert(bytes.length >= 8 && bytes.subarray(0, 4).equals(Buffer.from([0, 0x61, 0x73, 0x6d])), "artifact must be WebAssembly");
  let offset = 8;
  while (offset < bytes.length) {
    const sectionId = bytes[offset++];
    const cursor = { offset };
    const sectionSize = readU32Leb(bytes, cursor, bytes.length);
    offset = cursor.offset;
    const sectionEnd = offset + sectionSize;
    assert(sectionEnd <= bytes.length, "WebAssembly section must fit in module");
    if (sectionId === 5) {
      const sectionCursor = { offset };
      const count = readU32Leb(bytes, sectionCursor, sectionEnd);
      assert.equal(count, 1, "expected one defined WebAssembly memory");
      const flags = readU32Leb(bytes, sectionCursor, sectionEnd);
      assert.equal(flags, 1, "WebAssembly memory must declare a maximum");
      const minimumPages = readU32Leb(bytes, sectionCursor, sectionEnd);
      const maximumPages = readU32Leb(bytes, sectionCursor, sectionEnd);
      assert(minimumPages <= maximumPages, "WebAssembly minimum must not exceed its maximum");
      assert.equal(sectionCursor.offset, sectionEnd, "unexpected memory-section data");
      return BigInt(maximumPages) * 65_536n;
    }
    offset = sectionEnd;
  }
  throw new Error("Generated WebAssembly module has no defined-memory section.");
}

function readU32Leb(bytes, cursor, limit) {
  let value = 0;
  let shift = 0;
  for (let count = 0; count < 5; count += 1) {
    if (cursor.offset >= limit) throw new Error("Truncated WebAssembly unsigned integer.");
    const byte = bytes[cursor.offset++];
    if (count === 4 && (byte & 0xf0) !== 0) throw new Error("Invalid WebAssembly unsigned integer.");
    value += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return value;
    shift += 7;
  }
  throw new Error("Invalid WebAssembly unsigned integer.");
}
