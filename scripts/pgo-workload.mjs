import { createRequire } from 'node:module'
import { performance } from 'node:perf_hooks'
import { dirname, resolve, join } from 'node:path'
import { fileURLToPath } from 'node:url'
const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const native = createRequire(import.meta.url)(process.env.PGO_BINDING_PATH)
const training = process.argv[2] === 'train'
const results = {}
let sink = 0
async function measure(name, fn, iterations = 1) {
  for (let i = 0; i < 2; i++) await fn()
  if (!training) {
    const started = performance.now()
    for (let i = 0; i < iterations; i++) await fn()
    const elapsed = Math.max(performance.now() - started, 0.001)
    iterations = Math.max(iterations, Math.ceil((iterations * 30) / elapsed))
  }
  const samples = []
  for (let sample = 0; sample < (training ? 2 : 7); sample++) {
    const started = performance.now()
    for (let i = 0; i < iterations; i++) await fn()
    samples.push((performance.now() - started) / iterations)
  }
  results[name] = samples
}
const { readFileSync, mkdtempSync, rmSync } = await import('node:fs')
const { tmpdir } = await import('node:os')
const cache = mkdtempSync(join(tmpdir(), 'font-pgo-'))
let index = 0
try {
  for (const file of ['DejaVuSans.ttf', 'SyntheticCJK.ttf', 'SyntheticCJK.otf']) {
    const bytes = readFileSync(join(root, 'test', 'fixtures', file))
    const font = new native.FontSubsetter(bytes)
    const text = training ? 'Hello café — 中文字体 AV fi 1234' : 'The quick brown fox — 中文字形 AV fi 6789'
    const warmCache = join(cache, `${file}-warm`)
    font.subset(text, warmCache)
    await measure(
      `${file}.preparedCold`,
      () => {
        sink += font.subset(text, join(cache, String(index++))).bytes
      },
      5,
    )
    await measure(
      `${file}.preparedWarm`,
      () => {
        sink += font.subset(text, warmCache).bytes
      },
      30,
    )
    await measure(
      `${file}.asyncCold`,
      async () => {
        const batch = await Promise.all(
          Array.from({ length: 4 }, () => font.subsetAsync(text, join(cache, String(index++)))),
        )
        sink += batch[0].bytes
      },
      2,
    )
    await measure(
      `${file}.compatCold`,
      () => {
        sink += native.subsetFont(bytes, text, join(cache, String(index++))).bytes
      },
      3,
    )
    if (training) {
      const result = font.subset(text, join(cache, String(index++)))
      native.subsetFont(readFileSync(result.path), text, join(cache, String(index++)))
    }
  }
} finally {
  rmSync(cache, { recursive: true, force: true })
}
if (!Number.isFinite(sink)) throw new Error('Non-finite workload output')
console.log(JSON.stringify(results))
