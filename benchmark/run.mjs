import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { performance } from 'node:perf_hooks'
import subsetWasm from 'subset-font'
import { build } from 'vite'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'
const root = path.dirname(fileURLToPath(import.meta.url))
const fontPath = process.env.CJK_BENCH_FONT ?? path.join(root, '../test/fixtures/DejaVuSans.ttf')
const fontFaceIndex = Number(process.env.CJK_BENCH_FACE_INDEX ?? 0)
const font = await readFile(fontPath)
const corpus =
  Array.from({ length: 600 }, (_, i) => String.fromCodePoint(0x4e00 + i)).join('') +
  'The quick brown fox jumps over the lazy dog. 中文字体子集化性能测试页面内容缓存映射表'
const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-font-bench-'))
const measure = async (fn, times = 3) => {
  const samples = []
  let size = 0
  for (let i = 0; i < times; i++) {
    const start = performance.now()
    size = await fn(i)
    samples.push(performance.now() - start)
  }
  samples.sort((a, b) => a - b)
  return { medianMs: Number(samples[Math.floor(samples.length / 2)].toFixed(2)), outputBytes: size }
}

try {
  const nativeCold = await measure((i) => {
    const result = subsetFont(
      font,
      `${corpus}${String.fromCharCode(65 + i)}`,
      path.join(cache, `cold-${i}`),
      fontFaceIndex,
    )
    return result.bytes
  })
  const nativeWarm = await measure(() => subsetFont(font, corpus, path.join(cache, 'warm'), fontFaceIndex).bytes)
  subsetFont(font, corpus, path.join(cache, 'shared'), fontFaceIndex)
  let cacheHits = 0
  const samePageCache = await measure(() => {
    const result = subsetFont(font, corpus, path.join(cache, 'shared'), fontFaceIndex)
    if (result.cacheHit) cacheHits++
    return result.bytes
  })
  const wasm = await measure(async () => (await subsetWasm(font, corpus, { targetFormat: 'woff2' })).length)
  const uniqueStart = performance.now()
  const unique = Array.from(
    { length: 3 },
    (_, i) =>
      subsetFont(font, `${corpus}${String.fromCharCode(0x4e00 + i)}`, path.join(cache, 'unique'), fontFaceIndex).bytes,
  )
  const uniqueMs = Number((performance.now() - uniqueStart).toFixed(2))

  const viteRoot = path.join(cache, 'vite-root')
  await mkdir(viteRoot, { recursive: true })
  const pages = ['index.html', 'archive/index.html', 'tags/index.html']
  for (const page of pages) {
    const pagePath = path.join(viteRoot, page)
    await mkdir(path.dirname(pagePath), { recursive: true })
    await writeFile(pagePath, `<html><head></head><body><main>${corpus}</main></body></html>`)
  }
  const viteBuild = await measure(async (round) => {
    const outDir = path.join(viteRoot, `output-${round}`)
    await build({
      root: viteRoot,
      configFile: false,
      logLevel: 'silent',
      build: { outDir, rollupOptions: { input: pages.map((page) => path.join(viteRoot, page)) } },
      plugins: [
        cjkFontSplit({
          fonts: [{ src: fontPath, family: 'Bench Font', faceIndex: fontFaceIndex }],
          cacheDir: path.join(cache, 'vite-font-cache'),
        }),
      ],
    })
    return (await readFile(path.join(outDir, 'index.html'))).length
  })

  console.log(
    JSON.stringify(
      {
        node: process.version,
        platform: `${process.platform}-${process.arch}`,
        fontFile: path.basename(fontPath),
        fontBytes: font.length,
        fontFaceIndex,
        codepoints: [...corpus].length,
        repetitions: 3,
        nativeCold,
        nativeWarm,
        crossPageSameSubset: { medianMs: samePageCache.medianMs, cacheHitRate: `${cacheHits}/3 after warm-up` },
        nativeUniqueSets: { totalMs: uniqueMs, outputBytes: unique },
        subsetFontWasm: wasm,
        viteStaticMultiPageBuild: { medianMs: viteBuild.medianMs, pages: pages.length },
      },
      null,
      2,
    ),
  )
} finally {
  await rm(cache, { recursive: true, force: true })
}
