# @xingwangzhe/cjk-font-split-native

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Rust + N-API multilingual font subsetting for Vite, with a focus on CJK. It scans visible body text and CSS generated content, preserving each page's Latin letters, accented characters, punctuation, symbols, and CJK text. Head metadata, text attributes, scripts, and fully hidden content are excluded. Content-addressed caching reuses identical character sets across pages.

The package exposes an ESM-only API. Use `import`; CommonJS `require()` is not exported. NAPI-RS generates the native ESM loader and TypeScript declarations from the Rust API.

## Vite plugin

```ts
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

This plugin targets static multi-page builds. It extracts visible body text and CSS `content` strings from each HTML page and linked stylesheets, including the page's Latin letters, accented characters, punctuation, symbols, and CJK text. Head metadata and `alt`, `title`, `aria-label`, and other attribute values are excluded. Unique WOFF2 files are emitted into `assets/cjk-font-split/`, and page-scoped `@font-face` rules are injected. For client-rendered content, include its text in your HTML/prerender output or pass a complete corpus to the low-level API. Supported inputs are TTF, OTF, TTC (select a face with `faceIndex`), WOFF, and WOFF2; output is WOFF2. `family` must match the CSS font family used by the page. Optional `weight` and `style` default to `400` and `normal`.

## Native API

```ts
import { readFile } from 'node:fs/promises'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const result = subsetFont(
  await readFile('./LXGWWenKai-Regular.ttf'),
  'Text to retain',
  './.font-cache',
  0, // TTC faceIndex, optional
)
console.log(result) // { path, hash, cacheHit, bytes, characters }
```

The cache key is derived from the font's BLAKE3 digest, TTC face index, sorted unique code points, and subsetter/normalizer/compression algorithm versions. Cache files use this key as their name. WOFF2 compression quality is 8, selected based on the speed/size benchmark. An append-only `manifest.jsonl` maps keys to WOFF2 files. Writes use temporary files and atomic rename; identical inputs are reused across pages and builds.

## Development and release

Use Bun for JS dependencies and scripts, TypeScript 7 for Vite plugin types and declaration output, and Cargo for Rust dependencies and builds. `bun run ci` runs formatting, lint, TypeScript compilation, Rust tests and Clippy, the N-API release build, integration tests, and a benchmark. GitHub Actions builds and tests six targets; pushing a matching `vX.Y.Z` tag publishes the complete package through npm Trusted Publishing (OIDC) after CI passes. See the [workflow](.github/workflows/ci.yml).

For a full local release, download all six CI artifacts with `gh run download <run-id> --dir artifacts`, then run `bun run release:local`. The script validates the binaries, runs `bun pm pack --dry-run`, and publishes with Bun.

Run `bun run benchmark` to compare cold native subsets, warm cache hits, repeated subsets across pages, unique character sets, and `subset-font` WASM. It reports medians from three runs and output sizes; results depend on the hardware and are informational.

The DejaVu Sans test font lives in `test/fixtures/` under its upstream license and is excluded from the npm package.

## Reuse a font across pages

Use `FontSubsetter` for multi-page builds. Font normalization and hashing happen once in the constructor. Each subset reuses the prepared font. The existing `subsetFont()` API stays compatible; both APIs produce identical cache keys and WOFF2 bytes. The instance owns its font data, so later changes to the original Buffer do not affect it.

```ts
import { FontSubsetter } from '@xingwangzhe/cjk-font-split-native'

const prepared = new FontSubsetter(fontBuffer, 0)
const first = prepared.subset('First page 第一页', cacheDir)
const second = prepared.subset('Second page 第二页', cacheDir)
```

`await prepared.subsetAsync(text, cacheDir)` runs subsetting in the Node worker pool. Tasks share the prepared font, compression runs concurrently, and cache/manifest writes remain atomic and serialized. Use at most four concurrent tasks. The synchronous API remains compatible.

### Native backend and cache version

0.4.1 uses bundled HarfBuzz 14.6 for subsetting and Google Brotli 1.2 through a local woofwoof adapter for WOFF2. Quality remains 8 and font transforms remain enabled. `FontSubsetter` reuses preprocessed subset accelerators; a short mutex protects one font face while compression runs concurrently in the worker pool. WOFF/WOFF2 inputs are decoded to SFNT and TTC faces extracted before reusable HarfBuzz preprocessing. Original TrueType/CFF outlines are preserved without lossy curve conversion. No system HarfBuzz/Brotli installation is required.

The algorithm fingerprint changed, so old font caches are not reused. Filenames and WOFF2 bytes differ between 0.3.x and 0.4.1; synchronous, prepared and async APIs within 0.4.1 still produce matching results. HarfBuzz correctly rebuilds vertical metrics rather than retaining excess `vmtx` data. Bidirectional mirroring closure can add the corresponding mirrored symbols to the actual cmap. Third-party licenses are shipped with the npm package.
