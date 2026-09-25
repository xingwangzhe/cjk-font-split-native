import { readFile, writeFile } from 'node:fs/promises'
import { spawnSync } from 'node:child_process'

const manifestPath = new URL('../package.json', import.meta.url)
const original = await readFile(manifestPath, 'utf8')
const manifest = JSON.parse(original)

if (!manifest.version || manifest.version.includes('-')) {
  throw new Error('Expected the full release version in package.json before staging the placeholder.')
}

const localAddon = new URL('../cjk-font-split-native.linux-x64-gnu.node', import.meta.url)
try {
  await readFile(localAddon)
} catch {
  throw new Error('Build the Linux x64 GNU addon before publishing the placeholder.')
}

manifest.version = `${manifest.version}-preview.0`
manifest.os = ['linux']
manifest.cpu = ['x64']

try {
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`)

  const build = spawnSync('bun', ['run', 'build:ts'], { stdio: 'inherit' })
  if (build.error) throw build.error
  if (build.status !== 0) process.exit(build.status ?? 1)

  const pack = spawnSync('bun', ['pm', 'pack', '--dry-run'], { stdio: 'inherit' })
  if (pack.error) throw pack.error
  if (pack.status !== 0) process.exit(pack.status ?? 1)

  const publish = spawnSync('bun', ['publish', '--access', 'public', '--tag', 'linux-preview'], {
    stdio: 'inherit',
  })
  if (publish.error) throw publish.error
  if (publish.status !== 0) process.exitCode = publish.status ?? 1
} finally {
  await writeFile(manifestPath, original)
}
