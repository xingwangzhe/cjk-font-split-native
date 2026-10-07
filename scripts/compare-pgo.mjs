import { spawnSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const output = resolve(process.argv[2])
for (const stage of ['baseline', 'optimized']) {
  const warm = spawnSync(process.execPath, [join(root, 'scripts', 'pgo-workload.mjs'), 'benchmark'], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, PGO_BINDING_PATH: join(output, `${stage}.node`), RAYON_NUM_THREADS: '4' },
  })
  if (warm.error || warm.status !== 0) throw warm.error ?? new Error(warm.stderr)
}
const samples = { baseline: [], optimized: [] }
for (const stage of ['baseline', 'optimized', 'optimized', 'baseline', 'baseline', 'optimized']) {
  const result = spawnSync(process.execPath, [join(root, 'scripts', 'pgo-workload.mjs'), 'benchmark'], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, PGO_BINDING_PATH: join(output, `${stage}.node`), RAYON_NUM_THREADS: '4' },
  })
  if (result.error || result.status !== 0) throw result.error ?? new Error(result.stderr)
  samples[stage].push(JSON.parse(result.stdout))
}
const median = (values) => values.toSorted((a, b) => a - b)[Math.floor(values.length / 2)]
const workloads = Object.keys(samples.baseline[0])
const results = workloads.map((workload) => {
  const baselineMs = median(samples.baseline.flatMap((sample) => sample[workload]))
  const optimizedMs = median(samples.optimized.flatMap((sample) => sample[workload]))
  return { workload, baselineMs, optimizedMs, speedup: baselineMs / optimizedMs }
})
const report = {
  build: JSON.parse(readFileSync(join(output, 'build.json'), 'utf8')),
  threads: 4,
  processOrder: 'ABBAAB',
  results,
  geometricMeanSpeedup: Math.exp(results.reduce((sum, r) => sum + Math.log(r.speedup), 0) / results.length),
  samples,
}
writeFileSync(join(output, 'benchmark.json'), JSON.stringify(report, null, 2) + '\n')
console.info(JSON.stringify({ results, geometricMeanSpeedup: report.geometricMeanSpeedup }, null, 2))
