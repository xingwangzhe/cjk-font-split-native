import { createHash } from 'node:crypto'
import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
const args = process.argv.slice(2)
const take = (name) => {
  const index = args.indexOf(name)
  if (index < 0) return undefined
  if (!args[index + 1] || args[index + 1].startsWith('--')) throw new Error(`Missing value for ${name}`)
  return args.splice(index, 2)[1]
}
const capture = (command, values) => {
  const result = spawnSync(command, values, { cwd: root, encoding: 'utf8' })
  if (result.error || result.status !== 0) throw result.error ?? new Error(result.stderr)
  return result.stdout.trim()
}
const host = capture('rustc', ['-vV']).match(/^host: (.+)$/m)?.[1]
const target = take('--target') ?? host
const glibc = take('--zig-glibc')
const select = args.includes('--select')
if (select) args.splice(args.indexOf('--select'), 1)
const compare = select || args.includes('--compare')
if (args.includes('--compare')) args.splice(args.indexOf('--compare'), 1)
if (!target || !pkg.napi.targets.includes(target)) throw new Error(`Unsupported release target: ${target}`)
if (args.some((arg) => !['-x', '--use-napi-cross'].includes(arg))) throw new Error(`Unknown options: ${args.join(' ')}`)
const musl = target.endsWith('-musl')
if (target !== host && !(musl && host === 'x86_64-unknown-linux-gnu')) {
  throw new Error(`PGO training needs a matching runner: host=${host}, target=${target}`)
}
const labels = {
  'x86_64-unknown-linux-gnu': 'linux-x64-gnu',
  'aarch64-unknown-linux-gnu': 'linux-arm64-gnu',
  'x86_64-unknown-linux-musl': 'linux-x64-musl',
  'x86_64-apple-darwin': 'darwin-x64',
  'aarch64-apple-darwin': 'darwin-arm64',
  'x86_64-pc-windows-msvc': 'win32-x64-msvc',
}
const binary = join(root, `${pkg.napi.binaryName}.${labels[target]}.node`)
const output = join(root, 'target', 'pgo', target)
const raw = join(output, 'raw')
const profile = join(output, 'merged.profdata')
mkdirSync(output, { recursive: true })
rmSync(raw, { recursive: true, force: true })
mkdirSync(raw)
const sysroot = capture('rustc', ['--print', 'sysroot'])
const profdata = join(
  sysroot,
  'lib',
  'rustlib',
  host,
  'bin',
  `llvm-profdata${process.platform === 'win32' ? '.exe' : ''}`,
)
if (!existsSync(profdata)) throw new Error('Install the matching LLVM tools: rustup component add llvm-tools-preview')
const originalFlags =
  process.env.CARGO_ENCODED_RUSTFLAGS?.split('\x1f') ?? process.env.RUSTFLAGS?.trim().split(/\s+/).filter(Boolean) ?? []
if (originalFlags.some((arg) => /profile-(generate|use)/.test(arg)))
  throw new Error('Use build:pgo without external PGO flags')
const run = (command, values, env = {}) => {
  const result = spawnSync(command, values, { cwd: root, stdio: 'inherit', env: { ...process.env, ...env } })
  if (result.error || result.status !== 0) throw result.error ?? new Error(`${command} exited with ${result.status}`)
}
// Zig's driver rejects rustc's raw `-u SYMBOL` PGO runtime argument.
// Keep the selected Zig sysroot while translating that pair to -Wl,-u,SYMBOL.
const zigEnv =
  glibc || musl
    ? {
        PGO_REAL_ZIG: capture('sh', ['-c', 'command -v zig']),
        CARGO_ZIGBUILD_ZIG_PATH: join(root, 'scripts', 'zig-pgo.py'),
      }
    : {}
const stages = []
const build = (stage, flag) => {
  console.info(`PGO ${stage}: ${target}`)
  const env = {
    ...zigEnv,
    CARGO_INCREMENTAL: '0',
    CARGO_TARGET_DIR: join(root, 'target', 'pgo-build'),
    CARGO_ENCODED_RUSTFLAGS: [
      ...originalFlags,
      ...(musl ? ['-Ctarget-feature=-crt-static'] : []),
      ...(flag ? [flag] : []),
    ].join('\x1f'),
    RUSTFLAGS: '',
  }
  const started = Date.now()
  if (glibc) {
    if (!target.endsWith('-linux-gnu')) throw new Error('--zig-glibc requires a Linux GNU target')
    run('cargo', ['zigbuild', '--locked', '--release', '--target', `${target}.${glibc}`], env)
    const libName = pkg.napi.binaryName.replaceAll('-', '_')
    copyFileSync(join(env.CARGO_TARGET_DIR, target, 'release', `lib${libName}.so`), binary)
  } else {
    const napiArgs = ['x', 'napi', 'build', '--platform', '--release', '--target', target]
    if (pkg.napi.binaryName === 'mermaid-rs') napiArgs.push('--no-js')
    if (pkg.napi.binaryName === 'cjk-font-split-native')
      napiArgs.push('--esm', '--js', 'index.mjs', '--dts', 'index.d.mts')
    run('bun', [...napiArgs, ...args, '--', '--locked'], env)
  }
  if (!existsSync(binary)) throw new Error(`Missing native binary: ${binary}`)
  copyFileSync(binary, join(output, `${stage}.node`))
  stages.push({ stage, seconds: (Date.now() - started) / 1000 })
}
if (compare) build('baseline')
build('instrumented', `-Cprofile-generate=${raw}`)
const profilePattern = join(raw, '%m-%p.profraw')
if (musl) {
  run('docker', [
    'run',
    '--rm',
    '-v',
    `${root}:/work`,
    '-w',
    '/work',
    '-e',
    `LLVM_PROFILE_FILE=/work/target/pgo/${target}/raw/%m-%p.profraw`,
    '-e',
    `PGO_BINDING_PATH=/work/${pkg.napi.binaryName}.${labels[target]}.node`,
    '-e',
    'RAYON_NUM_THREADS=4',
    'node:24-alpine',
    'node',
    'scripts/pgo-workload.mjs',
    'train',
  ])
} else {
  run(process.execPath, ['scripts/pgo-workload.mjs', 'train'], {
    LLVM_PROFILE_FILE: profilePattern,
    PGO_BINDING_PATH: binary,
    RAYON_NUM_THREADS: '4',
  })
}
const profiles = readdirSync(raw)
  .filter((name) => name.endsWith('.profraw'))
  .map((name) => join(raw, name))
if (!profiles.length)
  throw new Error('Training did not produce any raw profiles; refusing to publish an untrained build')
run(profdata, ['merge', '-o', profile, ...profiles])
// A content-addressed profile path prevents Cargo from reusing code built
// with an older profile: Cargo does not track profile file contents itself.
const profileHash = createHash('sha256').update(readFileSync(profile)).digest('hex')
const versionedProfile = join(output, `merged-${profileHash}.profdata`)
copyFileSync(profile, versionedProfile)
build('optimized', `-Cprofile-use=${versionedProfile}`)
writeFileSync(
  join(output, 'build.json'),
  JSON.stringify(
    {
      package: pkg.name,
      version: pkg.version,
      target,
      rustc: capture('rustc', ['-vV']),
      profileCount: profiles.length,
      profileSha256: profileHash,
      stages,
    },
    null,
    2,
  ) + '\n',
)
console.info(`PGO release ready: ${binary}`)
if (compare) {
  if (musl) {
    run('docker', [
      'run',
      '--rm',
      '-v',
      `${root}:/work`,
      '-w',
      '/work',
      '-e',
      'RAYON_NUM_THREADS=4',
      'node:24-alpine',
      'node',
      'scripts/compare-pgo.mjs',
      `/work/target/pgo/${target}`,
    ])
  } else {
    run(process.execPath, ['scripts/compare-pgo.mjs', output])
  }
}
if (select) {
  const report = JSON.parse(readFileSync(join(output, 'benchmark.json'), 'utf8'))
  // Require a gain larger than minor timing noise, and avoid material
  // regressions in non-trivial workloads in this representative corpus.
  const improves = report.geometricMeanSpeedup >= 1.03
  const noLargeRegression = report.results.every((result) => result.baselineMs < 0.02 || result.speedup >= 0.85)
  const selected = improves && noLargeRegression ? 'optimized' : 'baseline'
  copyFileSync(join(output, `${selected}.node`), binary)
  const record = JSON.parse(readFileSync(join(output, 'build.json'), 'utf8'))
  record.selected = selected
  record.geometricMeanSpeedup = report.geometricMeanSpeedup
  record.selectionRule = 'PGO requires >=3% geometric mean gain and no >15% regression for workloads >=20us'
  writeFileSync(join(output, 'build.json'), JSON.stringify(record, null, 2) + '\n')
  console.info(`Selected ${selected} for publication: ${binary}`)
}
