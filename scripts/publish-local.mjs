import { spawnSync } from 'node:child_process'

const run = (command, args) => {
  const result = spawnSync(command, args, { stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}

run('bun', ['run', 'ci'])
run('node', ['scripts/assemble-artifacts.mjs'])
run('bun', ['pm', 'pack', '--dry-run'])
run('bun', ['publish', '--access', 'public'])
