import { spawnSync } from 'node:child_process'

const run = (command: string, args: string[]) => {
  const result = spawnSync(command, args, { stdio: 'inherit' })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}

run('bun', ['run', 'ci'])
run('bun', ['run', 'scripts/assemble-artifacts.ts'])
run('bunx', ['--no-install', 'napi', 'artifacts', '--output-dir', '.', '--npm-dir', 'npm'])
run('bun', ['pm', 'pack', '--dry-run'])
run('bunx', ['--no-install', 'napi', 'pre-publish', '--npm-dir', 'npm', '--tag-style', 'npm', '--no-gh-release'])
run('npm', ['publish', '--ignore-scripts', '--access', 'public'])
