import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const crateRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const workspaceRoot = resolve(crateRoot, "../..");
const targetRoot = process.env.CARGO_TARGET_DIR
  ? resolve(workspaceRoot, process.env.CARGO_TARGET_DIR)
  : resolve(workspaceRoot, "target");
const wasmPath = resolve(
  targetRoot,
  "wasm32-unknown-unknown/release/codegrid_wasm_server.wasm",
);
const fixtureRoot = resolve(workspaceRoot, "tests/fixtures");
const fixturePath = resolve(fixtureRoot, "conformance-v1.json");
const wasmBytes = await readFile(wasmPath);
const expectedMemoryBytes = configuredWasmMemoryLimit();
const actualMemoryBytes = readDefinedWasmMemoryLimit(wasmBytes);
assert.equal(actualMemoryBytes, expectedMemoryBytes, "server WASM must encode the configured linear-memory ceiling");
const module = await WebAssembly.compile(wasmBytes);
assert.deepEqual(WebAssembly.Module.imports(module), [], "server WASM must not import host functions");
const exports = WebAssembly.Module.exports(module).map(({ name }) => name);
for (const name of [
  "memory",
  "codegrid_server_abi_version",
  "codegrid_server_alloc_buffer",
  "codegrid_server_free_buffer",
  "codegrid_server_process_request",
]) {
  assert(exports.includes(name), `server WASM must export ${name}`);
}

const instance = await WebAssembly.instantiate(module, {});
const abi = instance.exports;
assert.equal(abi.codegrid_server_abi_version(), 4, "server ABI version must be 4");
assert.equal(abi.codegrid_server_alloc_buffer(0), 0, "zero-length buffers must be rejected");
assert.equal(abi.codegrid_server_alloc_buffer(8 * 1024 * 1024 + 1), 0, "oversized buffers must be rejected");
assert.equal(abi.codegrid_server_free_buffer(0), 0, "unknown pointers must not be freed");
assert.equal(
  abi.codegrid_server_process_request(0xffff_ffff, 1),
  0n,
  "unknown request pointers must be rejected",
);
const shortRequestPointer = abi.codegrid_server_alloc_buffer(4);
assert.notEqual(shortRequestPointer, 0, "short request buffer must be allocated");
assert.equal(
  abi.codegrid_server_process_request(shortRequestPointer, 5),
  0n,
  "request lengths beyond the allocated buffer must be rejected",
);
assert.equal(
  abi.codegrid_server_free_buffer(shortRequestPointer),
  1,
  "rejected request buffers must remain releasable",
);

const retainedBytePointers = [
  abi.codegrid_server_alloc_buffer(8 * 1024 * 1024),
  abi.codegrid_server_alloc_buffer(8 * 1024 * 1024),
];
assert(retainedBytePointers.every((pointer) => pointer !== 0), "two maximum buffers must fit the retained-byte ceiling");
assert.equal(abi.codegrid_server_alloc_buffer(1), 0, "retained buffers must obey the aggregate byte ceiling");
for (const pointer of retainedBytePointers) {
  assert.equal(abi.codegrid_server_free_buffer(pointer), 1, "maximum retained buffer must be releasable");
}

const retainedCountPointers = Array.from({ length: 1024 }, () => abi.codegrid_server_alloc_buffer(1));
assert(retainedCountPointers.every((pointer) => pointer !== 0), "the configured retained-buffer count must be available");
assert.equal(abi.codegrid_server_alloc_buffer(1), 0, "retained buffers must obey the aggregate count ceiling");
for (const pointer of retainedCountPointers) {
  assert.equal(abi.codegrid_server_free_buffer(pointer), 1, "retained buffer must be releasable");
}

function dispatch(request, targetAbi = abi, maximumResponseLength) {
  const payload = Buffer.from(JSON.stringify(request), "utf8");
  const requestPointer = targetAbi.codegrid_server_alloc_buffer(payload.length);
  assert.notEqual(requestPointer, 0, "valid request buffer must be allocated");
  let responsePointer = 0;
  try {
    new Uint8Array(targetAbi.memory.buffer, requestPointer, payload.length).set(payload);

    const packed = targetAbi.codegrid_server_process_request(requestPointer, payload.length);
    assert.notEqual(packed, 0n, "valid request must produce a response buffer");
    responsePointer = Number(packed >> 32n);
    const responseLength = Number(packed & 0xffff_ffffn);
    assert(responseLength > 0, "response must not be empty");
    if (maximumResponseLength !== undefined) {
      assert(
        responseLength <= maximumResponseLength,
        `response must fit its ${maximumResponseLength}-byte reservation`,
      );
    }
    const responseBytes = new Uint8Array(targetAbi.memory.buffer, responsePointer, responseLength);
    const response = JSON.parse(Buffer.from(responseBytes).toString("utf8"));
    assert.equal(targetAbi.codegrid_server_free_buffer(responsePointer), 1, "response buffer must be releasable");
    responsePointer = 0;
    return response;
  } finally {
    if (responsePointer !== 0) targetAbi.codegrid_server_free_buffer(responsePointer);
    assert.equal(targetAbi.codegrid_server_free_buffer(requestPointer), 1, "request buffer must be releasable");
  }
}

function assertResponseReservationFails(request, targetAbi = abi) {
  const payload = Buffer.from(JSON.stringify(request), "utf8");
  const requestPointer = targetAbi.codegrid_server_alloc_buffer(8 * 1024 * 1024);
  const retainedFillerPointer = targetAbi.codegrid_server_alloc_buffer(8 * 1024 * 1024);
  assert.notEqual(requestPointer, 0, "maximum request buffer must be allocated");
  assert.notEqual(retainedFillerPointer, 0, "maximum retained filler buffer must be allocated");
  try {
    new Uint8Array(targetAbi.memory.buffer, requestPointer, payload.length).set(payload);
    assert.equal(
      targetAbi.codegrid_server_process_request(requestPointer, payload.length),
      0n,
      "request must not be dispatched when its complete response cannot be reserved",
    );
  } finally {
    assert.equal(
      targetAbi.codegrid_server_free_buffer(retainedFillerPointer),
      1,
      "retained filler buffer must be releasable after reservation failure",
    );
    assert.equal(
      targetAbi.codegrid_server_free_buffer(requestPointer),
      1,
      "request buffer must be releasable after reservation failure",
    );
  }
}

function dispatchWithExactResponseReservation(request, targetAbi, responseCapacity) {
  const payload = Buffer.from(JSON.stringify(request), "utf8");
  const requestPointer = targetAbi.codegrid_server_alloc_buffer(payload.length);
  const firstFillerLength = 8 * 1024 * 1024;
  const secondFillerLength = 16 * 1024 * 1024 - payload.length - firstFillerLength - responseCapacity;
  assert(secondFillerLength > 0 && secondFillerLength <= 8 * 1024 * 1024, "response reservation must fit two bounded filler buffers");
  const firstFillerPointer = targetAbi.codegrid_server_alloc_buffer(firstFillerLength);
  const secondFillerPointer = targetAbi.codegrid_server_alloc_buffer(secondFillerLength);
  assert.notEqual(requestPointer, 0, "request buffer must be allocated");
  assert.notEqual(firstFillerPointer, 0, "first retained filler buffer must be allocated");
  assert.notEqual(secondFillerPointer, 0, "second retained filler buffer must be allocated");
  let responsePointer = 0;
  try {
    new Uint8Array(targetAbi.memory.buffer, requestPointer, payload.length).set(payload);
    const packed = targetAbi.codegrid_server_process_request(requestPointer, payload.length);
    assert.notEqual(packed, 0n, "configured response reservation must fit exactly");
    responsePointer = Number(packed >> 32n);
    const responseLength = Number(packed & 0xffff_ffffn);
    assert(responseLength > 0 && responseLength <= responseCapacity, "response must fit the configured response ceiling");
    const responseBytes = new Uint8Array(targetAbi.memory.buffer, responsePointer, responseLength);
    return JSON.parse(Buffer.from(responseBytes).toString("utf8"));
  } finally {
    if (responsePointer !== 0) {
      assert.equal(targetAbi.codegrid_server_free_buffer(responsePointer), 1, "reserved response buffer must be releasable");
    }
    assert.equal(targetAbi.codegrid_server_free_buffer(secondFillerPointer), 1, "second retained filler buffer must be releasable");
    assert.equal(targetAbi.codegrid_server_free_buffer(firstFillerPointer), 1, "first retained filler buffer must be releasable");
    assert.equal(targetAbi.codegrid_server_free_buffer(requestPointer), 1, "request buffer must be releasable");
  }
}

const fullRequestPointer = abi.codegrid_server_alloc_buffer(8 * 1024 * 1024);
const retainedFillerPointer = abi.codegrid_server_alloc_buffer(8 * 1024 * 1024);
assert.notEqual(fullRequestPointer, 0, "maximum request buffer must be allocated");
assert.notEqual(retainedFillerPointer, 0, "maximum filler buffer must be allocated");
const initializePayload = Buffer.from(
  JSON.stringify({
    abi_version: 4,
    api_version: 3,
    operation: "initialize",
    host_limits: hostLimits(),
  }),
  "utf8",
);
new Uint8Array(abi.memory.buffer, fullRequestPointer, initializePayload.length).set(initializePayload);
assert.equal(
  abi.codegrid_server_process_request(fullRequestPointer, initializePayload.length),
  0n,
  "response allocation must fail while the aggregate retained-buffer limit is full",
);
assert.equal(
  abi.codegrid_server_free_buffer(retainedFillerPointer),
  1,
  "the retained filler buffer must be releasable after response allocation fails",
);
assert.equal(
  abi.codegrid_server_free_buffer(fullRequestPointer),
  1,
  "the request buffer must remain releasable after response allocation fails",
);
const checkAfterResponseAllocationFailure = dispatch(
  {
    abi_version: 4,
    api_version: 3,
    operation: "check",
    source: "~x\n",
  },
  abi,
  256,
);
assert.equal(
  checkAfterResponseAllocationFailure.error.code,
  "runtime_not_initialized",
  "failed response reservation must not initialize the runtime",
);
const initialized = dispatch(
  {
    abi_version: 4,
    api_version: 3,
    operation: "initialize",
    host_limits: hostLimits(),
  },
  abi,
  256,
);
assert.equal(initialized.status, "initialized", "server runtime must initialize with trusted host limits");

const allocationRecoveryInstance = await WebAssembly.instantiate(module, {});
const allocationRecoveryAbi = allocationRecoveryInstance.exports;
const allocationRecoveryDispatch = (request) => dispatch(request, allocationRecoveryAbi);
const allocationRecoveryInitialized = allocationRecoveryDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "initialize",
  host_limits: {
    ...hostLimits(),
    max_compiled_programs: 1,
    max_instances: 1,
    max_response_bytes: 4096,
    max_instance_state_bytes: 2048,
  },
});
assert.equal(
  allocationRecoveryInitialized.status,
  "initialized",
  "a small configured response capacity must initialize the server session",
);

const allocationRecoverySource = "~> + ;\n";
const allocationRecoveryCompile = {
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: allocationRecoverySource,
};
assertResponseReservationFails(allocationRecoveryCompile, allocationRecoveryAbi);
const recoveredCompile = dispatchWithExactResponseReservation(
  allocationRecoveryCompile,
  allocationRecoveryAbi,
  4096,
);
assert.equal(recoveredCompile.status, "compiled", "failed compile response reservation must not consume a program slot");
assert.equal(recoveredCompile.program, "1", "compile retry must allocate the first program handle");

const allocationRecoveryCreate = {
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: recoveredCompile.program,
  input: [],
  boundary_mode: "exit",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [],
};
assertResponseReservationFails(allocationRecoveryCreate, allocationRecoveryAbi);
const recoveredCreate = allocationRecoveryDispatch(allocationRecoveryCreate);
assert.equal(recoveredCreate.instance, "1", "failed create response reservation must not consume an instance slot");

const allocationRecoveryStep = {
  abi_version: 4,
  api_version: 3,
  operation: "step",
  instance: recoveredCreate.instance,
};
assertResponseReservationFails(allocationRecoveryStep, allocationRecoveryAbi);
const stepAfterReservationFailure = allocationRecoveryDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "snapshot",
  instance: recoveredCreate.instance,
});
assert.equal(
  stepAfterReservationFailure.snapshot.committed_ticks,
  "0",
  "failed step response reservation must leave VM state unchanged",
);
const recoveredStep = allocationRecoveryDispatch(allocationRecoveryStep);
assert.equal(recoveredStep.result.committed_ticks, "1", "step retry must apply exactly one tick");

const allocationRecoveryRun = {
  abi_version: 4,
  api_version: 3,
  operation: "run",
  instance: recoveredCreate.instance,
  max_ticks: "10",
};
assertResponseReservationFails(allocationRecoveryRun, allocationRecoveryAbi);
const runAfterReservationFailure = allocationRecoveryDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "snapshot",
  instance: recoveredCreate.instance,
});
assert.equal(
  runAfterReservationFailure.snapshot.committed_ticks,
  "1",
  "failed run response reservation must preserve only previously committed ticks",
);
const recoveredRun = allocationRecoveryDispatch(allocationRecoveryRun);
assert.equal(recoveredRun.status, "halted", "run retry must complete the program");
assert.equal(recoveredRun.snapshot.committed_ticks, "3", "run retry must apply only the two remaining ticks");

const unsupportedAbi = dispatch({
  abi_version: 99,
  api_version: 3,
  operation: "check",
  source: "~> ;\n",
});
assert.equal(unsupportedAbi.error.code, "unsupported_abi_version");

const suite = JSON.parse(await readFile(fixturePath, "utf8"));
assert.equal(suite.schema_version, 1);
assert(suite.cases.length === 94, "the Full shared conformance suite must include all 94 runtime cases");
const simpleWrapSource = suite.cases.find((fixture) => fixture.id === "wrap-boundary-yields-after-bounded-run").source;

function assertPresentExpectedFields(fixture, result) {
  const expected = fixture.expected;
  const snapshot = result.snapshot;
  const threads = snapshot.threads;
  const errors = snapshot.errors;
  const actual = {
    error_custom_internal_ticks: errors
      .filter((error) => error.scope.kind === "custom")
      .map((error) => error.scope.internal_tick),
    error_global_ticks: errors.map((error) => error.global_tick),
    error_thread_id_groups: errors
      .map((error) => error.details.thread_ids ?? error.details.internal_thread_ids)
      .filter((ids) => ids !== undefined),
    error_thread_ids: errors
      .map((error) => error.details.thread_id)
      .filter((id) => id !== undefined),
    memory: snapshot.memory,
    remaining_input: snapshot.remaining_input,
    runtime_main_attachment_tokens: snapshot.runtime_program.main.cells.map((cell) => cell.attachment),
    runtime_main_primary_tokens: snapshot.runtime_program.main.cells.map((cell) => cell.primary),
    thread_call_stack_sizes: threads.map((thread) => thread.call_frames.length),
    thread_data_stack_sizes: threads.map((thread) => thread.data_stack.length),
    thread_direction: threads[0]?.direction,
    thread_instruction_stack_sizes: threads.map((thread) => thread.instruction_stack.length),
    thread_random_state: threads[0]?.random_state,
    threads: threads.map(({ id, position, direction }) => ({ id, position, direction })),
  };
  for (const [field, value] of Object.entries(actual)) {
    if (Object.hasOwn(expected, field)) {
      assert.deepEqual(value, expected[field], `${fixture.id}: ${field}`);
    }
  }
}

for (const fixture of suite.cases) {
  const compilation = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "compile",
    source: fixture.source,
  });
  assert.equal(compilation.status, "compiled", `${fixture.id}: compilation`);
  const viewedProgram = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "program_view",
    program: compilation.program,
  });
  assert.deepEqual(viewedProgram.view, compilation.view, `${fixture.id}: program_view returns canonical Full code`);
  const created = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "create_instance",
    program: compilation.program,
    input: fixture.run.input,
    boundary_mode: fixture.run.boundary,
    seed: fixture.run.seed,
    custom_execution_limit: fixture.run.custom_execution_limit,
    initial_memory: fixture.run.initial_memory ?? [],
  });
  const result = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "run",
    instance: created.instance,
    max_ticks: fixture.run.max_ticks,
  });
  const repeatedCreated = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "create_instance",
    program: compilation.program,
    input: fixture.run.input,
    boundary_mode: fixture.run.boundary,
    seed: fixture.run.seed,
    custom_execution_limit: fixture.run.custom_execution_limit,
    initial_memory: fixture.run.initial_memory ?? [],
  });
  const repeatedResult = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "run",
    instance: repeatedCreated.instance,
    max_ticks: fixture.run.max_ticks,
  });
  assert.deepEqual(repeatedResult.snapshot, result.snapshot, `${fixture.id}: deterministic repeated run`);
  const expected = fixture.expected;
  assert.equal(result.abi_version, 4, `${fixture.id}: server ABI version`);
  assert.equal(result.api_version, 3, `${fixture.id}: Runtime API version`);
  assert.equal(result.operation, "run", `${fixture.id}: operation`);
  assert.equal(result.status, expected.status, `${fixture.id}: run status`);
  assert.equal(result.snapshot.status, expected.vm_status, `${fixture.id}: VM status`);
  assert.deepEqual(result.snapshot.registers, expected.registers, `${fixture.id}: registers`);
  assert.deepEqual(result.snapshot.output, expected.output, `${fixture.id}: output`);
  assert.equal(result.snapshot.committed_ticks, expected.committed_ticks, `${fixture.id}: committed ticks`);
  for (const metric of ["operation_count", "used_cell_count", "used_memory_address_count", "peak_data_stack_usage", "peak_instruction_stack_usage", "peak_call_stack_usage"]) {
    assert.equal(result.snapshot.metrics[metric], expected[metric], `${fixture.id}: ${metric}`);
  }
  assert.deepEqual(result.snapshot.metrics.instruction_variety, expected.instruction_variety, `${fixture.id}: instruction variety`);
  assert.deepEqual(
    result.snapshot.errors.map((error) => error.code),
    expected.error_codes,
    `${fixture.id}: ordered error codes`,
  );
  assert(Array.isArray(result.events), `${fixture.id}: run returns the committed event sequence`);
  assert(Array.isArray(result.newly_emitted_output), `${fixture.id}: run returns output delta`);
  assertPresentExpectedFields(fixture, result);
  const releasedInstance = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "release_instance",
    instance: created.instance,
  });
  assert.equal(releasedInstance.operation, "release_instance", `${fixture.id}: release instance`);
  const releasedRepeatedInstance = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "release_instance",
    instance: repeatedCreated.instance,
  });
  assert.equal(releasedRepeatedInstance.operation, "release_instance", `${fixture.id}: release repeated instance`);
  const releasedProgram = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "release_program",
    program: compilation.program,
  });
  assert.equal(releasedProgram.operation, "release_program", `${fixture.id}: release program`);
}

const limitedInstance = await WebAssembly.instantiate(module, {});
const limitedAbi = limitedInstance.exports;
const limitedDispatch = (request) => dispatch(request, limitedAbi);
const limitedInit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "initialize",
  host_limits: {
    ...hostLimits(),
    max_source_bytes: 64,
    max_compiled_programs: 1,
    max_instances: 2,
    max_input_bytes: 1,
    max_initial_memory_entries: 1,
    max_run_ticks_per_call: "4",
    max_total_ticks_per_instance: "2",
    max_work_units_per_call: "1",
  },
});
assert.equal(limitedInit.status, "initialized", "all accepted host ceilings must initialize");
const sourceOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: `${" ".repeat(65)}`,
});
assert.equal(sourceOverLimit.error.code, "source_payload_limit_exceeded", "source bytes must obey the configured ceiling");
assert.equal(sourceOverLimit.error.error_number, "7508", "server transport alias retains its scoped numeric identity");
const limitedProgram = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: simpleWrapSource,
});
assert.equal(limitedProgram.status, "compiled");
assert.equal(typeof limitedProgram.program, "string", "program handles must use exact decimal strings");
const programOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: simpleWrapSource,
});
assert.equal(programOverLimit.error.code, "program_limit_reached", "compiled-program count must obey its configured ceiling");
const inputOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: limitedProgram.program,
  input: [1, 2],
  boundary_mode: "wrap",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [],
});
assert.equal(inputOverLimit.error.code, "input_payload_limit_exceeded", "input bytes must obey the configured ceiling");
const initialMemoryOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: limitedProgram.program,
  input: [],
  boundary_mode: "wrap",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [
    { address: "-1", value: 7 },
    { address: "2", value: 9 },
  ],
});
assert.equal(initialMemoryOverLimit.error.code, "initial_memory_limit_exceeded", "initial memory entries must obey their configured ceiling");
const limitedCreate = () => limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: limitedProgram.program,
  input: [],
  boundary_mode: "wrap",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [],
});
const workLimited = limitedCreate();
const totalLimited = limitedCreate();
assert.equal(typeof workLimited.instance, "string", "instance handles must use exact decimal strings");
assert.equal(limitedCreate().error.code, "instance_limit_reached", "instance count must obey its configured ceiling");
const perCallOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "run",
  instance: workLimited.instance,
  max_ticks: "5",
});
assert.equal(perCallOverLimit.error.code, "run_tick_budget_exceeded", "per-call ticks must obey their configured ceiling");
const workYield = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "run",
  instance: workLimited.instance,
  max_ticks: "2",
});
assert.equal(workYield.status, "work_limit_reached", "a work ceiling must yield a bounded run");
assert.equal(workYield.snapshot.committed_ticks, "1", "work exhaustion must preserve complete earlier ticks");
assert.equal(typeof workYield.snapshot.metrics.operation_count, "string", "tick metrics must use exact decimal strings");
for (let tick = 0; tick < 2; tick += 1) {
  const result = limitedDispatch({
    abi_version: 4,
    api_version: 3,
    operation: "run",
    instance: totalLimited.instance,
    max_ticks: "1",
  });
  assert.equal(result.snapshot.committed_ticks, String(tick + 1), "total tick limits must be scoped to each instance");
}
const totalOverLimit = limitedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "run",
  instance: totalLimited.instance,
  max_ticks: "1",
});
assert.equal(totalOverLimit.error.code, "instance_tick_budget_exceeded", "total ticks must obey their configured ceiling");

const boundedResponseInstance = await WebAssembly.instantiate(module, {});
const boundedResponseDispatch = (request) => dispatch(request, boundedResponseInstance.exports);
const boundedResponseInit = boundedResponseDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "initialize",
  host_limits: {
    ...hostLimits(),
    max_response_bytes: 1024,
    max_instance_state_bytes: 256,
  },
});
assert.equal(boundedResponseInit.status, "initialized");
const oversizedDiagnostics = boundedResponseDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "check",
  source: "~x\n".repeat(100),
});
assert.equal(oversizedDiagnostics.error.code, "response_payload_limit_exceeded", "response serialization must return a complete bounded error");
const boundedProgram = boundedResponseDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: "~> ;\n",
});
assert.equal(boundedProgram.status, "compiled");
const oversizedState = boundedResponseDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: boundedProgram.program,
  input: [],
  boundary_mode: "exit",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [],
});
assert.equal(oversizedState.error.code, "instance_state_limit_exceeded", "retained snapshot state must obey its configured ceiling");

const lifecycleProgram = dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: "~> , . ;\n",
});
assert.equal(lifecycleProgram.status, "compiled", "lifecycle source must compile");
function createLifecycleInstance(input) {
  return dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "create_instance",
    program: lifecycleProgram.program,
    input,
    boundary_mode: "exit",
    seed: "0",
    custom_execution_limit: "100",
    initial_memory: [],
  });
}
const firstInstance = createLifecycleInstance([41]);
const secondInstance = createLifecycleInstance([99]);

const isolatedModule = await WebAssembly.instantiate(module, {});
const isolatedAbi = isolatedModule.exports;
const isolatedDispatch = (request) => dispatch(request, isolatedAbi);
const isolatedInit = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "initialize",
  host_limits: { ...hostLimits(), max_compiled_programs: 1, max_instances: 1 },
});
assert.equal(isolatedInit.status, "initialized", "a second module instance must initialize independently");
const isolatedProgram = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: "~> , . ;\n",
});
assert.equal(isolatedProgram.status, "compiled", "the second module must own an independent program registry");
const isolatedCreated = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "create_instance",
  program: isolatedProgram.program,
  input: [7],
  boundary_mode: "exit",
  seed: "0",
  custom_execution_limit: "100",
  initial_memory: [],
});
const isolatedResult = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "run",
  instance: isolatedCreated.instance,
  max_ticks: "10",
});
assert.deepEqual(isolatedResult.snapshot.output, [7], "the second module must execute its own instance state");
const foreignProgramHandle = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "release_program",
  program: lifecycleProgram.program,
});
assert.equal(foreignProgramHandle.error.code, "unknown_program_handle", "program handles must not cross module sessions");
const isolatedOverLimit = isolatedDispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: "~> , . ;\n",
});
assert.equal(isolatedOverLimit.error.code, "program_limit_reached", "the second module must enforce its own configured quota");
const primaryWithinLimit = dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "compile",
  source: "~> , . ;\n",
});
assert.equal(primaryWithinLimit.status, "compiled", "another module's quota must not restrict this module");
assert.equal(
  dispatch({ abi_version: 4, api_version: 3, operation: "release_program", program: primaryWithinLimit.program }).operation,
  "release_program",
);
const isolatedShutdown = isolatedDispatch({ abi_version: 4, api_version: 3, operation: "shutdown" });
assert.equal(isolatedShutdown.status, "closed", "the second module must shut down independently");

const firstStep = dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "step",
  instance: firstInstance.instance,
});
assert.equal(firstStep.result.attempted_tick, "1", "step must report its attempted outer tick");
assert.equal(firstStep.result.committed_ticks, "1", "step must advance exactly one outer tick");
assert(Array.isArray(firstStep.result.newly_emitted_output), "step must expose newly emitted output bytes");
assert(Array.isArray(firstStep.result.errors), "step must expose structured runtime errors");
assert(Array.isArray(firstStep.result.events), "step must return ordered committed events");
assert(firstStep.result.snapshot?.threads?.length > 0, "step must include a Full post-transition snapshot");
assert.deepEqual(
  Object.keys(firstStep.result).sort(),
  ["attempted_tick", "committed_ticks", "errors", "events", "fault", "metrics", "newly_emitted_output", "snapshot", "status"].sort(),
  "step result must expose Full state and events",
);
const firstSnapshot = dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "snapshot",
  instance: firstInstance.instance,
});
const secondSnapshot = dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "snapshot",
  instance: secondInstance.instance,
});
assert.equal(firstSnapshot.snapshot.committed_ticks, "1", "first instance state must persist across calls");
assert.equal(secondSnapshot.snapshot.committed_ticks, "0", "second instance state must remain isolated");
assert(Array.isArray(firstSnapshot.snapshot.threads), "snapshot must expose Full per-thread details");
assert.equal(
  dispatch({ abi_version: 4, api_version: 3, operation: "snapshot", instance: firstInstance.instance }).snapshot.committed_ticks,
  "1",
  "shutting down another module must not affect this module's live instances",
);
dispatch({
  abi_version: 4,
  api_version: 3,
  operation: "release_program",
  program: lifecycleProgram.program,
});
for (const [created, expectedByte] of [[firstInstance, 41], [secondInstance, 99]]) {
  const result = dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "run",
    instance: created.instance,
    max_ticks: "10",
  });
  assert.deepEqual(result.snapshot.output, [expectedByte], "released program must not invalidate created instances");
  dispatch({
    abi_version: 4,
    api_version: 3,
    operation: "release_instance",
    instance: created.instance,
  });
}
const shutdown = dispatch({ abi_version: 4, api_version: 3, operation: "shutdown" });
assert.equal(shutdown.status, "closed", "runtime shutdown must release its session");

const maximumPages = Number(actualMemoryBytes / 65_536n);
const currentPages = abi.memory.buffer.byteLength / 65_536;
assert(currentPages <= maximumPages, "server WASM memory must not exceed its declared maximum");
assert.equal(abi.memory.grow(maximumPages - currentPages), currentPages, "server memory must grow to its configured maximum");
const maximumMemoryBuffer = abi.memory.buffer;
assert.throws(() => abi.memory.grow(1), RangeError, "server memory growth above the configured maximum must fail");
assert.equal(abi.memory.buffer, maximumMemoryBuffer, "failed over-limit growth must not replace the memory buffer");

process.stdout.write(`PASS: no-import server WASM ABI and ${suite.cases.length} shared fixtures in Node WebAssembly.\n`);

function hostLimits() {
  return {
    max_source_bytes: 4 * 1024 * 1024,
    max_compiled_programs: 8,
    max_instances: 8,
    max_input_bytes: 1024 * 1024,
    max_initial_memory_entries: 65536,
    max_run_ticks_per_call: "1000000",
    max_total_ticks_per_instance: "10000000",
    max_work_units_per_call: "1000000",
    max_response_bytes: 8 * 1024 * 1024,
    max_instance_state_bytes: 8 * 1024 * 1024,
  };
}

function configuredWasmMemoryLimit() {
  const value = process.env.CODEGRID_WASM_MAX_MEMORY_BYTES;
  assert(value && /^\d+$/.test(value), "set CODEGRID_WASM_MAX_MEMORY_BYTES to verify the server memory ceiling");
  const maximum = BigInt(value);
  assert(maximum > 0n && maximum <= 4_294_967_296n && maximum % 65_536n === 0n, "configured server memory ceiling must be page-aligned and in range");
  return maximum;
}

function readDefinedWasmMemoryLimit(bytes) {
  assert(bytes.length >= 8 && bytes[0] === 0 && bytes[1] === 0x61 && bytes[2] === 0x73 && bytes[3] === 0x6d, "server artifact must be a WebAssembly module");
  let offset = 8;
  while (offset < bytes.length) {
    const sectionId = bytes[offset++];
    const cursor = { offset };
    const sectionSize = readU32Leb(bytes, cursor, bytes.length);
    offset = cursor.offset;
    const sectionEnd = offset + sectionSize;
    assert(sectionEnd <= bytes.length, "server WASM section length must be valid");
    if (sectionId === 5) {
      const sectionCursor = { offset };
      assert.equal(readU32Leb(bytes, sectionCursor, sectionEnd), 1, "server WASM must define exactly one memory");
      assert.equal(readU32Leb(bytes, sectionCursor, sectionEnd), 1, "server WASM memory must declare a maximum");
      const minimumPages = readU32Leb(bytes, sectionCursor, sectionEnd);
      const maximumPages = readU32Leb(bytes, sectionCursor, sectionEnd);
      assert(minimumPages <= maximumPages, "server WASM memory minimum must not exceed its maximum");
      assert.equal(sectionCursor.offset, sectionEnd, "server WASM memory section must not contain extra data");
      return BigInt(maximumPages) * 65_536n;
    }
    offset = sectionEnd;
  }
  throw new Error("server WASM must define a memory section");
}

function readU32Leb(bytes, cursor, limit) {
  let value = 0;
  let shift = 0;
  for (let count = 0; count < 5; count += 1) {
    assert(cursor.offset < limit, "WASM unsigned integer must not be truncated");
    const byte = bytes[cursor.offset++];
    assert(!(count === 4 && (byte & 0xf0) !== 0), "WASM unsigned integer must fit u32");
    value += (byte & 0x7f) * 2 ** shift;
    if ((byte & 0x80) === 0) return value;
    shift += 7;
  }
  throw new Error("invalid WASM unsigned integer");
}
