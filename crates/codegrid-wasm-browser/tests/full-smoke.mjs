export const RUNTIME_API_VERSION = 3;

export function runFullSmoke(BrowserRuntime, suite, baseline, assert, deepEqual) {
  assert(suite.schema_version === 1, "the shared Full fixture suite must use schema version 1");
  assert(Array.isArray(suite.cases) && suite.cases.length === 75, "all 75 Full fixtures must be available");
  assert(baseline?.schema === "codegrid.native-cli.full-run-baseline", "the generated Native CLI baseline must be available");
  assert(baseline.schema_version === 1 && baseline.api_version === RUNTIME_API_VERSION, "the Native CLI baseline must target Runtime API v3");
  assert(baseline.suite_id === suite.suite_id, "the Native CLI baseline must belong to this fixture suite");
  assert(baseline.max_work_units_per_call === "1000000", "the baseline must use the shared deterministic work-unit ceiling");
  assert(Array.isArray(baseline.cases) && baseline.cases.length === suite.cases.length, "the Native CLI baseline must cover every Full fixture");
  deepEqual(baseline.cases.map(({ id }) => id), suite.cases.map(({ id }) => id), "Native CLI baseline fixture IDs");

  const runtime = createRuntime(BrowserRuntime);
  assert(runtime.api_version() === RUNTIME_API_VERSION, "the adapter must expose Runtime API v3");

  for (const fixture of suite.cases) {
    const { id, source, run, expected } = fixture;
    const compiled = parseJson(runtime.compile(source), `${id}: compile`);
    assertSuccess(compiled, `${id}: compile`, assert);
    assert(compiled.outcome.kind === "compiled", `${id}: Full source must compile`);
    assert(typeof compiled.outcome.program === "string", `${id}: program handle is a decimal string`);
    assert(!("view" in compiled.outcome), `${id}: compile returns a handle without a copied view`);

    const view = parseJson(runtime.program_view(compiled.outcome.program), `${id}: program view`);
    assertSuccess(view, `${id}: program view`, assert);
    assert(view.view && view.view.outer && view.view.customs, `${id}: view contains the Full program`);

    const configuration = JSON.stringify({
      boundary_mode: run.boundary,
      seed: run.seed,
      custom_execution_limit: run.custom_execution_limit,
    });
    const initialMemory = JSON.stringify(run.initial_memory ?? []);
    const created = parseJson(
      runtime.create_instance(
        compiled.outcome.program,
        new Uint8Array(run.input),
        initialMemory,
        configuration,
      ),
      `${id}: create instance`,
    );
    assertSuccess(created, `${id}: create instance`, assert);
    assert(typeof created.instance === "string", `${id}: instance handle is a decimal string`);

    const result = parseJson(runtime.run(created.instance, run.max_ticks), `${id}: run`);
    assertSuccess(result, `${id}: run`, assert);
    const expectedStatus = expected.status === "tick_limit_reached"
      ? { kind: "yielded", reason: "tick_slice_exhausted" }
      : { kind: expected.status };
    deepEqual(result.status, expectedStatus, `${id}: run status`);
    deepEqual(result.newly_emitted_output, expected.output, `${id}: newly emitted output`);
    assert(Array.isArray(result.events), `${id}: run returns structured events`);
    assertFullSnapshot(result.snapshot, expected, id, assert, deepEqual);
    const baselineResult = baseline.cases.find((entry) => entry.id === id)?.result;
    assert(baselineResult, `${id}: complete Native CLI result is present in the baseline`);
    deepEqual(normalizeBrowserResult(result), baselineResult, `${id}: complete Browser WASM / Native CLI result`);

    const releasedInstance = parseJson(runtime.release_instance(created.instance), `${id}: release instance`);
    assertSuccess(releasedInstance, `${id}: release instance`, assert);
    const releasedProgram = parseJson(runtime.release_program(compiled.outcome.program), `${id}: release program`);
    assertSuccess(releasedProgram, `${id}: release program`, assert);
  }

  verifyHostBoundaries(BrowserRuntime, assert, deepEqual);
  return suite.cases.length;
}

function normalizeBrowserResult(result) {
  const status = result.status?.kind === "yielded"
    ? (result.status.reason === "tick_slice_exhausted" ? "tick_limit_reached" : "work_limit_reached")
    : result.status?.kind;
  if (!["halted", "error", "tick_limit_reached", "work_limit_reached"].includes(status)) {
    throw new Error(`Unsupported Browser Runtime API run status: ${JSON.stringify(result.status)}`);
  }
  return {
    status,
    events: result.events.map(normalizeEvent),
    newly_emitted_output: result.newly_emitted_output,
    snapshot: normalizeSnapshot(result.snapshot),
  };
}

function normalizeSnapshot(snapshot) {
  return {
    status: snapshot.status,
    committed_ticks: snapshot.committed_ticks,
    registers: snapshot.registers,
    memory: snapshot.memory,
    remaining_input: snapshot.remaining_input,
    output: snapshot.output,
    runtime_program: normalizeRuntimeProgram(snapshot.runtime_program),
    threads: snapshot.threads.map((thread) => normalizeThread(thread, true)),
    metrics: {
      ...snapshot.metrics,
      used_cells: snapshot.metrics.used_cells.map(normalizeStaticCell),
    },
    errors: snapshot.errors.map(normalizeError),
    fault: snapshot.fault,
  };
}

function normalizeRuntimeProgram(program) {
  return {
    main: normalizeRuntimeBoard(program.main),
    functions: Object.fromEntries(Object.entries(program.functions).map(([id, board]) => [id, normalizeRuntimeBoard(board)])),
  };
}

function normalizeRuntimeBoard(board) {
  return {
    width: canonicalDimension(board.width),
    height: canonicalDimension(board.height),
    cells: board.cells.map((cell) => ({
      entry: cell.entry === null ? null : cell.entry.replace(/^~/, ""),
      primary: cell.primary,
      prefix: cell.prefix,
      attachment: cell.attachment,
    })),
    folded_blocks: board.folded_blocks,
  };
}

function canonicalDimension(value) {
  if (typeof value !== "string" || !/^(0|[1-9][0-9]*)$/.test(value)) {
    throw new Error(`Runtime API board dimension is not a canonical decimal: ${JSON.stringify(value)}`);
  }
  const dimension = Number(value);
  if (!Number.isSafeInteger(dimension) || dimension > 65_535) {
    throw new Error(`Runtime API board dimension is outside its model range: ${value}`);
  }
  return dimension;
}

function normalizeEvent(event) {
  const normalized = { ...event };
  if (event.cell) normalized.cell = normalizeStaticCell(event.cell);
  if (event.location) normalized.location = normalizeMemoryLocation(event.location);
  if (event.kind === "thread_changed") {
    // API thread-change payloads include a derived code-grid ID. CLI event
    // payloads omit it, so validate its shape and compare the shared state.
    normalized.before = normalizeThread(event.before, false);
    normalized.after = normalizeThread(event.after, false);
  }
  return normalized;
}

function normalizeThread(thread, includeCodeGrid) {
  return {
    ...(includeCodeGrid ? { code_grid: normalizeCodeGrid(thread.code_grid) } : {}),
    id: thread.id,
    board: normalizeBoard(thread.board),
    position: thread.position,
    direction: thread.direction,
    register_pointer: thread.register_pointer,
    page: thread.page,
    data_stack: thread.data_stack,
    instruction_stack: thread.instruction_stack,
    call_frames: thread.call_frames.map((frame) => ({
      caller_board: normalizeBoard(frame.caller_board),
      call_position: frame.call_position,
      saved_direction: frame.saved_direction,
    })),
    phase: thread.phase,
    random_state: thread.random_state,
  };
}

function normalizeError(error) {
  const details = { ...error.details };
  if (details.board) details.board = normalizeBoard(details.board);
  if (details.cell) details.cell = normalizeStaticCell(details.cell);
  return { code: error.code, error_number: error.error_number, global_tick: error.global_tick, scope: error.scope, details };
}

function normalizeStaticCell(cell) {
  return {
    code_grid: normalizeCodeGrid(cell.code_grid),
    board: normalizeBoard(cell.board),
    folded_block: cell.folded_block,
    position: cell.position,
  };
}

function normalizeMemoryLocation(location) {
  return { space: location.space, address: location.address };
}

function normalizeCodeGrid(id) {
  if (typeof id === "string") return id;
  return id.kind === "outer" ? "outer" : `custom:${id.id}`;
}

function normalizeBoard(id) {
  if (typeof id === "string") return id;
  return id.kind === "main" ? "main" : `function:${id.id}`;
}

function createRuntime(Runtime, options = {}) {
  return new Runtime(
    options.maxSourceBytes ?? 1_048_576,
    options.maxPrograms ?? 128,
    options.maxInstances ?? 128,
    options.maxInputBytes ?? 1_048_576,
    options.maxInitialMemoryEntries ?? 65_536,
    options.maxRequestBytes ?? 1_048_576,
    options.maxResponseBytes ?? 1_048_576,
    options.maxInstanceStateBytes ?? 1_048_576,
    options.maxRunTicksPerCall ?? "10000",
    options.maxTotalTicksPerInstance ?? "10000",
    options.maxWorkUnitsPerCall ?? "1000000",
  );
}

function parseJson(value, operation) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`${operation} returned invalid JSON: ${error}`);
  }
}

function assertSuccess(response, operation, assert) {
  assert(response.api_version === RUNTIME_API_VERSION && !response.error, `${operation}: expected Runtime API v3 success`);
}

function scalarOrArray(actual, expected, assert, operation) {
  const normalized = Array.isArray(expected)
    ? actual
    : (assert(actual.length === 1, `${operation}: scalar fixture requires one value`), actual[0]);
  return normalized;
}

function assertFullSnapshot(snapshot, expected, id, assert, deepEqual) {
  assert(snapshot.status === expected.vm_status, `${id}: VM status`);
  for (const key of ["committed_ticks", "registers", "output"]) {
    deepEqual(snapshot[key], expected[key], `${id}: ${key}`);
  }
  for (const key of [
    "operation_count",
    "used_cell_count",
    "used_memory_address_count",
    "peak_data_stack_usage",
    "peak_instruction_stack_usage",
    "peak_call_stack_usage",
    "instruction_variety",
  ]) {
    deepEqual(snapshot.metrics[key], expected[key], `${id}: metrics.${key}`);
  }
  deepEqual(snapshot.errors.map((error) => error.code), expected.error_codes, `${id}: error codes`);
  for (const error of snapshot.errors) {
    assert(/^3[0-9]{3}$/.test(error.error_number), `${id}: four-digit VM error number`);
    if (error.code === "ConcurrentOutputConflict") assert(error.error_number === "3006", `${id}: stable output-conflict number`);
  }

  if ("remaining_input" in expected) deepEqual(snapshot.remaining_input, expected.remaining_input, `${id}: remaining input`);
  if ("memory" in expected) deepEqual(snapshot.memory, expected.memory, `${id}: memory`);
  if ("thread_direction" in expected) assert(snapshot.threads[0].direction === expected.thread_direction, `${id}: thread direction`);
  if ("thread_random_state" in expected) assert(snapshot.threads[0].random_state === expected.thread_random_state, `${id}: thread random state`);
  if ("threads" in expected) {
    const actual = snapshot.threads.map((thread) => ({
      id: thread.id,
      position: thread.position,
      direction: thread.direction,
    }));
    deepEqual(scalarOrArray(actual, expected.threads, assert, `${id}: threads`), expected.threads, `${id}: threads`);
  }

  for (const [expectedKey, field] of [
    ["thread_call_stack_sizes", "call_frames"],
    ["thread_data_stack_sizes", "data_stack"],
    ["thread_instruction_stack_sizes", "instruction_stack"],
  ]) {
    if (expectedKey in expected) {
      const actual = snapshot.threads.map((thread) => thread[field].length);
      deepEqual(scalarOrArray(actual, expected[expectedKey], assert, `${id}: ${expectedKey}`), expected[expectedKey], `${id}: ${expectedKey}`);
    }
  }

  for (const [key, field] of [
    ["runtime_main_primary_tokens", "primary"],
    ["runtime_main_attachment_tokens", "attachment"],
  ]) {
    if (key in expected) {
      deepEqual(snapshot.runtime_program.main.cells.map((cell) => cell[field]), expected[key], `${id}: ${key}`);
    }
  }

  for (const [key, select] of [
    ["error_global_ticks", (error) => error.global_tick],
    ["error_custom_internal_ticks", (error) => error.scope.internal_tick],
    ["error_thread_ids", (error) => error.details.thread_id],
  ]) {
    if (key in expected) {
      const actual = snapshot.errors.map(select).filter((value) => value !== undefined);
      deepEqual(scalarOrArray(actual, expected[key], assert, `${id}: ${key}`), expected[key], `${id}: ${key}`);
    }
  }
  if ("error_thread_id_groups" in expected) {
    const actual = snapshot.errors
      .map((error) => error.details.thread_ids ?? error.details.internal_thread_ids)
      .filter((value) => value !== undefined);
    deepEqual(actual, expected.error_thread_id_groups, `${id}: error thread-id groups`);
  }
}

function verifyHostBoundaries(BrowserRuntime, assert, deepEqual) {
  const programRuntime = createRuntime(BrowserRuntime);
  const compiled = parseJson(programRuntime.compile("~> ,v . ;\n"), "compile host-boundary source");
  assertSuccess(compiled, "compile host-boundary source", assert);

  const disguised = new Uint16Array([73]);
  Object.defineProperty(disguised, Symbol.toStringTag, { value: "Uint8Array" });
  const rejected = parseJson(
    programRuntime.create_instance(
      compiled.outcome.program,
      disguised,
      "[]",
      JSON.stringify({ boundary_mode: "exit", seed: "0", custom_execution_limit: "100" }),
    ),
    "reject disguised Uint8Array",
  );
  assert(rejected.error.code === "invalid_input", "typed-array brands cannot be spoofed");

  const created = parseJson(
    programRuntime.create_instance(
      compiled.outcome.program,
      new Uint8Array([73]),
      "[]",
      JSON.stringify({ boundary_mode: "exit", seed: "9007199254740993", custom_execution_limit: "100" }),
    ),
    "create isolated instance",
  );
  assertSuccess(created, "create isolated instance", assert);
  const result = parseJson(programRuntime.run(created.instance, "10"), "run isolated instance");
  deepEqual(result.snapshot.output, [73], "isolated instance output");
  assert(Array.isArray(result.snapshot.threads), "halted instances retain a structured thread snapshot");
  assertSuccess(parseJson(programRuntime.release_instance(created.instance), "release isolated instance"), "release isolated instance", assert);

  const requestGuard = createRuntime(BrowserRuntime, { maxRequestBytes: 16 });
  const requestError = parseJson(
    requestGuard.create_instance(
      "1",
      new Uint8Array(32),
      "[]",
      JSON.stringify({ boundary_mode: "exit", seed: "0", custom_execution_limit: "10" }),
    ),
    "request byte quota",
  );
  assert(requestError.error.code === "request_payload_limit_exceeded", "create payload quota must count all request fields before copying");

  const inputGuard = createRuntime(BrowserRuntime, { maxInputBytes: 1 });
  const inputProgram = parseJson(inputGuard.compile("~> ;\n"), "compile input-limited source");
  assertSuccess(inputProgram, "compile input-limited source", assert);
  const inputError = parseJson(
    inputGuard.create_instance(
      inputProgram.outcome.program,
      new Uint8Array([1, 2]),
      "[]",
      JSON.stringify({ boundary_mode: "exit", seed: "0", custom_execution_limit: "10" }),
    ),
    "input byte quota",
  );
  assert(inputError.error.code === "input_payload_limit_exceeded", "browser input bytes must be rejected before copying into WASM");
  assertSuccess(parseJson(inputGuard.release_program(inputProgram.outcome.program), "release input-limited program"), "release input-limited program", assert);

  const memoryGuard = createRuntime(BrowserRuntime, { maxInitialMemoryEntries: 1 });
  const memoryProgram = parseJson(memoryGuard.compile("~> ;\n"), "compile initial-memory fixture");
  assertSuccess(memoryProgram, "compile initial-memory fixture", assert);
  const config = JSON.stringify({ boundary_mode: "exit", seed: "0", custom_execution_limit: "10" });
  const duplicateMemory = parseJson(
    memoryGuard.create_instance(
      memoryProgram.outcome.program,
      new Uint8Array(),
      JSON.stringify([{ address: "1", value: 1 }, { address: "2", value: 2 }]),
      config,
    ),
    "initial-memory entry limit",
  );
  assert(duplicateMemory.error.code === "initial_memory_limit_exceeded", "initial-memory entry quota must be enforced before conversion");
  const invalidConfig = parseJson(
    memoryGuard.create_instance(memoryProgram.outcome.program, new Uint8Array(), "[]", JSON.stringify({ boundary_mode: "exit", seed: "01", custom_execution_limit: "10" })),
    "noncanonical configuration integer",
  );
  assert(invalidConfig.error.code === "invalid_configuration_integer", "wide request integers must be canonical decimal strings");
  assertSuccess(parseJson(memoryGuard.release_program(memoryProgram.outcome.program), "release initial-memory program"), "release initial-memory program", assert);

  const duplicateGuard = createRuntime(BrowserRuntime, { maxInitialMemoryEntries: 2 });
  const duplicateProgram = parseJson(duplicateGuard.compile("~> ;\n"), "compile duplicate-memory fixture");
  assertSuccess(duplicateProgram, "compile duplicate-memory fixture", assert);
  const duplicateAddress = parseJson(
    duplicateGuard.create_instance(
      duplicateProgram.outcome.program,
      new Uint8Array(),
      JSON.stringify([{ address: "9007199254740993", value: 1 }, { address: "9007199254740993", value: 2 }]),
      config,
    ),
    "duplicate initial-memory address",
  );
  assert(duplicateAddress.error.code === "duplicate_initial_memory_address", "duplicate addresses must be rejected after exact parsing");
  assertSuccess(parseJson(duplicateGuard.release_program(duplicateProgram.outcome.program), "release duplicate-memory program"), "release duplicate-memory program", assert);

  const sourceGuard = createRuntime(BrowserRuntime, { maxSourceBytes: 5 });
  const sourceError = parseJson(sourceGuard.check("ééé"), "UTF-8 source quota");
  assert(sourceError.error.code === "source_payload_limit_exceeded", "source quota must count UTF-8 bytes");

  const tickGuard = createRuntime(BrowserRuntime, { maxRunTicksPerCall: "2", maxTotalTicksPerInstance: "2" });
  const tickProgram = parseJson(tickGuard.compile("~> + ;\n"), "compile tick-limited source");
  assertSuccess(tickProgram, "compile tick-limited source", assert);
  const tickInstance = parseJson(
    tickGuard.create_instance(
      tickProgram.outcome.program,
      new Uint8Array(),
      "[]",
      JSON.stringify({ boundary_mode: "wrap", seed: "0", custom_execution_limit: "10" }),
    ),
    "create tick-limited instance",
  );
  assertSuccess(tickInstance, "create tick-limited instance", assert);
  const rejectedSlice = parseJson(tickGuard.run(tickInstance.instance, "3"), "reject oversized run slice");
  assert(rejectedSlice.error.code === "run_tick_budget_exceeded", "oversized run slices must fail before progress");
  const unchanged = parseJson(tickGuard.snapshot(tickInstance.instance), "snapshot after rejected slice");
  assert(unchanged.snapshot.committed_ticks === "0", "rejected slices must not advance an instance");
  assertSuccess(parseJson(tickGuard.release_instance(tickInstance.instance), "release tick-limited instance"), "release tick-limited instance", assert);
  assertSuccess(parseJson(tickGuard.release_program(tickProgram.outcome.program), "release tick-limited program"), "release tick-limited program", assert);

  const workGuard = createRuntime(BrowserRuntime, { maxWorkUnitsPerCall: "1" });
  const workProgram = parseJson(workGuard.compile("~> + ;\n"), "compile work-limited source");
  assertSuccess(workProgram, "compile work-limited source", assert);
  const workInstance = parseJson(
    workGuard.create_instance(
      workProgram.outcome.program,
      new Uint8Array(),
      "[]",
      JSON.stringify({ boundary_mode: "wrap", seed: "0", custom_execution_limit: "10" }),
    ),
    "create work-limited instance",
  );
  assertSuccess(workInstance, "create work-limited instance", assert);
  const workYield = parseJson(workGuard.run(workInstance.instance, "10"), "work-limited run");
  deepEqual(workYield.status, { kind: "yielded", reason: "work_unit_budget_exhausted" }, "work-unit yield reason");
  assert(Array.isArray(workYield.events) && Array.isArray(workYield.newly_emitted_output), "work-limited run retains structured committed deltas");
  assertSuccess(parseJson(workGuard.release_instance(workInstance.instance), "release work-limited instance"), "release work-limited instance", assert);
  assertSuccess(parseJson(workGuard.release_program(workProgram.outcome.program), "release work-limited program"), "release work-limited program", assert);

  const responseGuard = createRuntime(BrowserRuntime, { maxResponseBytes: 256 });
  const largeSource = `~> ${"_ ".repeat(300)}`;
  const largeProgram = parseJson(responseGuard.compile(largeSource), "compile compact handle response");
  assertSuccess(largeProgram, "compile compact handle response", assert);
  const responseError = parseJson(responseGuard.program_view(largeProgram.outcome.program), "bounded Full program view");
  assert(responseError.error.code === "response_payload_limit_exceeded", "oversized projections must return a complete response limit error");

  const stateGuard = createRuntime(BrowserRuntime, {
    maxRunTicksPerCall: "5000",
    maxTotalTicksPerInstance: "5000",
    maxInstanceStateBytes: 4096,
  });
  const outputProgram = parseJson(stateGuard.compile("@main\n@size 2x1\n~> .\n@end\n"), "compile retained-state fixture");
  assertSuccess(outputProgram, "compile retained-state fixture", assert);
  const outputInstance = parseJson(
    stateGuard.create_instance(
      outputProgram.outcome.program,
      new Uint8Array(),
      "[]",
      JSON.stringify({ boundary_mode: "wrap", seed: "0", custom_execution_limit: "100" }),
    ),
    "create retained-state fixture",
  );
  assertSuccess(outputInstance, "create retained-state fixture", assert);
  const stateError = parseJson(stateGuard.run(outputInstance.instance, "5000"), "retained-state quota");
  assert(stateError.error.code === "instance_state_limit_exceeded", "over-quota snapshots must release their instance");
  const discarded = parseJson(stateGuard.snapshot(outputInstance.instance), "snapshot discarded instance");
  assert(discarded.error.code === "unknown_instance_handle", "over-quota instances must no longer be accessible");
}
