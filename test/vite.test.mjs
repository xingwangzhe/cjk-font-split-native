import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm, mkdir, writeFile, readdir } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { build } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

const rootDir = path.dirname(fileURLToPath(import.meta.url))
test('Vite plugin emits one subset for pages with identical text and injects CSS', async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'cjk-vite-'))
  try {
    await mkdir(path.join(root, 'src'))
    const fontPath = path.join(rootDir, 'fixtures/DejaVuSans.ttf')
    const pageHtml =
      '<html><head><style>h1::after{content:"附加文字"}</style></head><body><h1>Same page</h1></body></html>'
    await writeFile(path.join(root, 'index.html'), pageHtml)
    await writeFile(path.join(root, 'other.html'), pageHtml)
    await build({
      root,
      configFile: false,
      logLevel: 'silent',
      build: { rollupOptions: { input: [path.join(root, 'index.html'), path.join(root, 'other.html')] } },
      plugins: [cjkFontSplit({ fonts: [{ src: fontPath, family: 'DejaVu' }], cacheDir: path.join(root, '.cache') })],
    })
    const index = await readFile(path.join(root, 'dist/index.html'), 'utf8')
    const other = await readFile(path.join(root, 'dist/other.html'), 'utf8')
    assert.match(index, /data-cjk-font-split/)
    assert.match(index, /\.\/assets\/cjk-font-split\/[a-f\d]+\.woff2/)
    const assets = await readdir(path.join(root, 'dist/assets/cjk-font-split'))
    assert.equal(assets.length, 1)
    assert.match(other, new RegExp(assets[0]))
  } finally {
    await rm(root, { recursive: true, force: true })
  }
})
