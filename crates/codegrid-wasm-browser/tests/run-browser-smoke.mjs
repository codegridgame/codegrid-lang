import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { createReadStream, existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const workspaceRoot = resolve(scriptDirectory, "../../..");
const targetDirectory = resolve(workspaceRoot, process.env.CARGO_TARGET_DIR ?? "target");
const generatedBindings = resolve(targetDirectory, "browser-bindings");
const generatedWasm = resolve(generatedBindings, "codegrid_wasm_browser_bg.wasm");
const baselinePath = resolve(targetDirectory, "wasmtime-host/native-cli-full-baseline-v1.json");
const browserSmokePage = resolve(scriptDirectory, "browser-smoke.html");
const browserPath = process.env.CODEGRID_BROWSER ?? findBrowser();
const bindingRelativePath = relative(workspaceRoot, resolve(generatedBindings, "codegrid_wasm_browser.js"));
if (bindingRelativePath === ".." || bindingRelativePath.startsWith(`..${sep}`) || isAbsolute(bindingRelativePath)) {
  throw new Error("CARGO_TARGET_DIR must resolve to a directory inside the workspace for browser smoke.");
}
const baselineRelativePath = relative(workspaceRoot, baselinePath);
if (baselineRelativePath === ".." || baselineRelativePath.startsWith(`..${sep}`) || isAbsolute(baselineRelativePath)) {
  throw new Error("CARGO_TARGET_DIR must resolve to a directory inside the workspace for browser smoke.");
}

for (const requiredPath of [
  resolve(generatedBindings, "codegrid_wasm_browser.js"),
  generatedWasm,
  browserSmokePage,
  resolve(scriptDirectory, "full-smoke.mjs"),
  resolve(workspaceRoot, "tests/fixtures/conformance-v1.json"),
  baselinePath,
]) {
  if (!existsSync(requiredPath)) throw new Error(`Missing browser smoke input: ${requiredPath}`);
}

const expectedWasmMemoryBytes = configuredWasmMemoryLimit();
const actualWasmMemoryBytes = readDefinedWasmMemoryLimit(readFileSync(generatedWasm));
assert.equal(
  actualWasmMemoryBytes,
  expectedWasmMemoryBytes,
  "generated browser WASM must encode CODEGRID_WASM_MAX_MEMORY_BYTES as its maximum",
);

const mimeTypes = new Map([
  [".cg", "text/plain; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".mjs", "text/javascript; charset=utf-8"],
  [".wasm", "application/wasm"],
]);

const server = createServer((request, response) => {
  let requestPath;
  try {
    requestPath = decodeURIComponent(new URL(request.url, "http://127.0.0.1").pathname);
  } catch {
    response.writeHead(400).end("Invalid request path");
    return;
  }

  const filePath = resolve(workspaceRoot, `.${requestPath}`);
  if (filePath !== workspaceRoot && !filePath.startsWith(`${workspaceRoot}${sep}`)) {
    response.writeHead(403).end("Forbidden");
    return;
  }
  if (!existsSync(filePath) || !statSync(filePath).isFile()) {
    response.writeHead(404).end("Not found");
    return;
  }

  response.writeHead(200, {
    "Content-Type": mimeTypes.get(extname(filePath)) ?? "application/octet-stream",
    "Cache-Control": "no-store",
  });
  createReadStream(filePath).pipe(response);
});

const profileDirectory = mkdtempSync(join(tmpdir(), "codegrid-wasm-browser-smoke-"));

try {
  const address = await new Promise((resolveAddress, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => resolveAddress(server.address()));
  });
  const memoryMaximumPages = expectedWasmMemoryBytes / 65_536n;
  const query = new URLSearchParams({
    memoryMaximumPages: memoryMaximumPages.toString(),
    bindingsPath: `/${bindingRelativePath.split(sep).join("/")}`,
    baselinePath: `/${baselineRelativePath.split(sep).join("/")}`,
  });
  const url = `http://127.0.0.1:${address.port}/crates/codegrid-wasm-browser/tests/browser-smoke.html?${query}`;
  const { stdout, stderr } = await runBrowser(url, profileDirectory);
  if (!stdout.includes('data-result="passed"')) {
    const result = stdout.match(/<pre id="result">([\s\S]*?)<\/pre>/)?.[1];
    throw new Error(`Browser Full smoke failed: ${result ?? stdout.slice(-2000)}\n${stderr}`);
  }
  const result = stdout.match(/<pre id="result">([^<]*)<\/pre>/)?.[1];
  assert.match(result ?? "", /^PASS: browser WebAssembly Runtime API v3/);
  console.log(`${result} Configured WASM memory maximum verified at ${expectedWasmMemoryBytes} bytes.`);
} finally {
  await new Promise((resolveClose) => server.close(resolveClose));
  const resolvedProfile = resolve(profileDirectory);
  const resolvedTemp = resolve(tmpdir());
  if (
    resolvedProfile.startsWith(`${resolvedTemp}${sep}`) &&
    basename(resolvedProfile).startsWith("codegrid-wasm-browser-smoke-")
  ) {
    rmSync(resolvedProfile, { recursive: true, force: true });
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
      assert(minimumPages <= maximumPages, "WebAssembly minimum memory must not exceed its maximum");
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

function findBrowser() {
  const candidates = process.platform === "win32"
    ? [
        join(process.env.PROGRAMFILES ?? "C:\\Program Files", "Google/Chrome/Application/chrome.exe"),
        join(process.env["PROGRAMFILES(X86)"] ?? "C:\\Program Files (x86)", "Microsoft/Edge/Application/msedge.exe"),
        join(process.env.LOCALAPPDATA ?? "", "Microsoft/Edge/Application/msedge.exe"),
      ]
    : process.platform === "darwin"
      ? [
          "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
          "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ]
      : [
          "/usr/bin/google-chrome",
          "/usr/bin/google-chrome-stable",
          "/opt/google/chrome/chrome",
          "/usr/bin/chromium",
          "/usr/bin/chromium-browser",
          "/usr/bin/microsoft-edge",
        ];
  const browser = candidates.find((candidate) => candidate && existsSync(candidate));
  if (!browser) throw new Error("Chrome or Edge was not found; set CODEGRID_BROWSER to the browser executable path.");
  return browser;
}

function runBrowser(url, profile) {
  return new Promise((resolveOutput, reject) => {
    const child = spawn(browserPath, [
      `--user-data-dir=${profile}`,
      "--headless",
      "--disable-gpu",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-background-networking",
      "--virtual-time-budget=15000",
      "--dump-dom",
      url,
    ], { windowsHide: true });

    let stdout = "";
    let stderr = "";
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error("Headless browser smoke exceeded 45 seconds."));
    }, 45_000);
    child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
    child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code) => {
      clearTimeout(timeout);
      if (code === 0) resolveOutput({ stdout, stderr });
      else reject(new Error(`Browser exited with code ${code}.\n${stderr}\n${stdout}`));
    });
  });
}
