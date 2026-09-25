# @xingwangzhe/cjk-font-split-native

Rust + N-API CJK font subsetting for Vite. It scans each emitted HTML page, creates a WOFF2 subset, injects a page-scoped `@font-face`, and reuses content-addressed output when pages need the same characters.

## Vite

```js
import { defineConfig } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

export default defineConfig({
  plugins: [
    cjkFontSplit({
      fonts: [{ src: './src/fonts/LXGWWenKai-Regular.ttf', family: 'LXGW WenKai', weight: '400' }],
      // Optional; defaults to Vite's cacheDir/cjk-font-split-native.
      cacheDir: './node_modules/.vite/cjk-font-split-native',
    }),
  ],
})
```

This is a build-only plugin for static multi-page output. It extracts rendered text and CSS `content` strings from each HTML page and its linked stylesheets, emits unique WOFF2 files into `assets/cjk-font-split/`, and injects `@font-face` rules. For apps with client-rendered content, pass that text through your HTML/prerender output or use the low-level API to subset an explicit text corpus.

Supported input formats are TTF, OTF, TTC (with `faceIndex`), WOFF, and WOFF2. Output is WOFF2. `family` must match the CSS family used by the page. Optional `weight` and `style` describe the face; defaults are `400` and `normal`.

## Native API

```js
import { readFile } from 'node:fs/promises'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const result = subsetFont(
  await readFile('./LXGWWenKai-Regular.ttf'),
  '这段文本会被保留',
  './.font-cache',
  0, // TTC faceIndex, optional
)
console.log(result) // { path, hash, cacheHit, bytes, characters }
```

Cache files are named by BLAKE3 over font bytes, TTC face index, sorted unique codepoints, and the subsetter/normalizer/compression version. WOFF2 uses compression quality 8, selected after benchmarking its speed/size tradeoff. An append-only `manifest.jsonl` maps each key to its WOFF2 file. Cache writes use temporary files and atomic rename; repeated keys reuse the same artifact across pages/builds.

## Development

Use Bun for JavaScript packages and scripts, Cargo for Rust dependencies/builds. `bun run ci` runs formatting, lint, Clippy, release N-API build, JS integration tests, and a benchmark. The CI target matrix builds and uploads the six supported desktop/server targets and deliberately has no publish/OIDC job. After downloading those artifacts with `gh run download <run-id> --dir artifacts`, `bun run release:local` validates all six binaries, runs `bun pm pack --dry-run`, and publishes once with Bun.

`bun run benchmark` compares cold native subsets, warm cache hits, repeated page subsets, unique subsets, and `subset-font` WASM on the same font/text. It prints medians from three runs and output sizes; timings depend on hardware and are informational.

The bundled DejaVu Sans test fixture is under its upstream license in `test/fixtures/DEJAVU-LICENSE.txt`; it is excluded from npm package contents.
