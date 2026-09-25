import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, mkdir, writeFile, readdir } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { build } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

const rootDir = path.dirname(fileURLToPath(import.meta.url))
test('Vite plugin subsets visible Latin, symbols, and CJK while excluding metadata and hidden text', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'cjk-vite-'))
  try {
    await mkdir(path.join(root, 'src'))
    const fontPath = path.join(rootDir, 'fixtures/DejaVuSans.ttf')
    const visibleBody =
      '<main><h1>English café — “quotes” © 2026 中文</h1><button aria-label="属性值">Go</button><p hidden>隐藏内容</p><p class="sr-only">屏幕阅读器文本</p></main>'
    const pageHtml = `<html><head><title>Head title</title><meta name="description" content="头部描述"><style>h1::after{content:" — symbols ✓"}</style></head><body>${visibleBody}</body></html>`
    const sameVisibleTextDifferentMetadata = `<html><head><title>Other title</title><meta name="description" content="不同头部"><style>h1::after{content:" — symbols ✓"}</style></head><body><main><h1>English café — “quotes” © 2026 中文</h1><button aria-label="另一个属性">Go</button><p hidden>另一个隐藏内容</p><p class="sr-only">另一个屏幕阅读器文本</p></main></body></html>`
    const extraVisibleLatin = pageHtml.replace('2026 中文', '2026Z 中文')
    await writeFile(path.join(root, 'index.html'), pageHtml)
    await writeFile(path.join(root, 'other.html'), sameVisibleTextDifferentMetadata)
    await writeFile(path.join(root, 'latin.html'), extraVisibleLatin)
    await build({
      root,
      configFile: false,
      logLevel: 'silent',
      build: {
        rollupOptions: {
          input: [path.join(root, 'index.html'), path.join(root, 'other.html'), path.join(root, 'latin.html')],
        },
      },
      plugins: [
        cjkFontSplit({
          fonts: [{ src: fontPath, family: 'DejaVu' }],
          cacheDir: path.join(root, '.cache'),
        }),
      ],
    })
    const index = await readFile(path.join(root, 'dist/index.html'), 'utf8')
    const other = await readFile(path.join(root, 'dist/other.html'), 'utf8')
    const latin = await readFile(path.join(root, 'dist/latin.html'), 'utf8')
    assert.match(index, /data-cjk-font-split/)
    assert.match(index, /\.\/assets\/cjk-font-split\/[a-f\d]+\.woff2/)
    const indexSubset = index.match(/\.\/assets\/cjk-font-split\/([a-f\d]+\.woff2)/)?.[1]
    const otherSubset = other.match(/\.\/assets\/cjk-font-split\/([a-f\d]+\.woff2)/)?.[1]
    assert.ok(indexSubset)
    assert.equal(otherSubset, indexSubset)
    assert.doesNotMatch(latin, new RegExp(indexSubset))
    const assets = await readdir(path.join(root, 'dist/assets/cjk-font-split'))
    assert.equal(assets.length, 2)
    assert.ok(assets.includes(indexSubset))
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
