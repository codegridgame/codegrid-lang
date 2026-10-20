import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { resolve, relative } from 'node:path'

const root = resolve(import.meta.dirname, '..')
const directory = resolve(root, 'target/browser-bindings')
const version = (command, args) => execFileSync(command, args, { cwd: root, encoding: 'utf8' }).trim()
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex')
const files = directory => readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
  const path = resolve(directory, entry.name)
  return entry.isDirectory() ? files(path) : [path]
})
const bytes = Number(process.env.CODEGRID_WASM_MAX_MEMORY_BYTES)
if (!Number.isSafeInteger(bytes) || bytes <= 0 || bytes % 65536 !== 0) throw new Error('Provide CODEGRID_WASM_MAX_MEMORY_BYTES')
const manifest = {
  schema_version: 1,
  source_commit: version('git', ['rev-parse', 'HEAD']),
  source_dirty: version('git', ['status', '--porcelain']).length > 0,
  runtime_api_version: 5,
  wasm_bindgen_version: version('wasm-bindgen', ['--version']).split(' ')[1],
  wasm32_linear_memory_ceiling_bytes: bytes,
  files_sha256: Object.fromEntries(files(directory).filter(path => /\.(js|wasm)$/.test(path)).sort().map(path => [relative(directory, path).replaceAll('\\', '/'), hash(path)])),
}
writeFileSync(resolve(root, 'target/browser-build-provenance.json'), `${JSON.stringify(manifest, null, 2)}\n`)
