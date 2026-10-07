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

0.4.2 uses bundled HarfBuzz 14.6 for subsetting and Google Brotli 1.2 through a local woofwoof adapter for WOFF2. Quality remains 8 and font transforms remain enabled. `FontSubsetter` reuses preprocessed subset accelerators; a short mutex protects one font face while compression runs concurrently in the worker pool. WOFF/WOFF2 inputs are decoded to SFNT and TTC faces extracted before reusable HarfBuzz preprocessing. Original TrueType/CFF outlines are preserved without lossy curve conversion. No system HarfBuzz/Brotli installation is required. The Vite plugin shares each subset-copy promise and linked-CSS read promise between concurrent pages, avoiding same-target file contention on Windows.

The algorithm fingerprint changed, so old font caches are not reused. Filenames and WOFF2 bytes differ between 0.3.x and 0.4.2; synchronous, prepared and async APIs within 0.4.2 still produce matching results. HarfBuzz correctly rebuilds vertical metrics rather than retaining excess `vmtx` data. Bidirectional mirroring closure can add the corresponding mirrored symbols to the actual cmap. Third-party licenses are shipped with the npm package.

## LICENSES and scope

The project’s Rust / JavaScript code is **MIT**. The root [LICENSE](LICENSE) retains the standard text for GitHub detection. The files below apply to their respective third-party components, **not as additional licenses for the entire project**. Upstream `OR` expressions retain their alternative choices. Exact component versions, copyrights, original licenses and NOTICE files are collected in [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt).

| 组件 / Component                                       | 许可 / License                                  | 全文 / Full text                                              |
| ------------------------------------------------------ | ----------------------------------------------- | ------------------------------------------------------------- |
| Project Rust / JS code                                 | MIT                                             | [LICENSE-MIT.txt](LICENSES/LICENSE-MIT.txt)                   |
| HarfBuzz 14.6                                          | MIT-style (original text retained)              | [LICENSE-HARFBUZZ.txt](LICENSES/LICENSE-HARFBUZZ.txt)         |
| Google WOFF2                                           | MIT                                             | [LICENSE-WOFF2.txt](LICENSES/LICENSE-WOFF2.txt)               |
| Google Brotli 1.2                                      | MIT                                             | [LICENSE-BROTLI.txt](LICENSES/LICENSE-BROTLI.txt)             |
| compu-brotli-sys                                       | Boost Software License 1.0                      | [LICENSE-BOOST-1.0.txt](LICENSES/LICENSE-BOOST-1.0.txt)       |
| allsorts and Unicode category/combining/joining crates | Apache-2.0                                      | [LICENSE-APACHE-2.0.txt](LICENSES/LICENSE-APACHE-2.0.txt)     |
| alloc-no-stdlib / alloc-stdlib                         | BSD-3-Clause                                    | [LICENSE-BSD-3-Clause.txt](LICENSES/LICENSE-BSD-3-Clause.txt) |
| libloading                                             | ISC                                             | [LICENSE-ISC.txt](LICENSES/LICENSE-ISC.txt)                   |
| Unicode data in unicode-ident                          | Unicode-3.0                                     | [LICENSE-UNICODE-3.0.txt](LICENSES/LICENSE-UNICODE-3.0.txt)   |
| License option for blake3 and related crates           | CC0-1.0                                         | [LICENSE-CC0-1.0.txt](LICENSES/LICENSE-CC0-1.0.txt)           |
| License option for constant_time_eq                    | Embedded LLVM runtime (Zig Linux builds)        | Apache-2.0 WITH LLVM-exception; legacy notices retained       | [LICENSE-LLVM-RUNTIME.txt](LICENSES/LICENSE-LLVM-RUNTIME.txt) |
| MIT-0                                                  | [LICENSE-MIT-0.txt](LICENSES/LICENSE-MIT-0.txt) |
| License option for tinyvec                             | Zlib                                            | [LICENSE-ZLIB.txt](LICENSES/LICENSE-ZLIB.txt)                 |
| License option for adler2                              | 0BSD                                            | [LICENSE-0BSD.txt](LICENSES/LICENSE-0BSD.txt)                 |

The npm package ships `LICENSES/` and the complete third-party notice file. Input fonts and generated subsets retain their font-specific licenses, reserved names and redistribution conditions; this tool’s MIT does not replace them. Test fonts retain their own licenses and are excluded from npm. GitHub’s primary-license display is separate from this component inventory.

## Runtime-focused native release builds

CI release binaries use O3, full LTO, one code-generation unit, and disabled incremental compilation. CI trains and measures a profile-guided optimization (PGO) candidate on each platform, selecting it only when the comparison passes; otherwise it publishes the unprofiled release. The existing CPU instruction baseline is preserved. Use `bun run build:pgo` for the optimized native build and `bun run benchmark:pgo` for a comparison against an unprofiled release. Install the matching LLVM tools with `rustup component add llvm-tools-preview`. Training inputs, measurement limits, and platform requirements are documented in [scripts/PERFORMANCE.md](scripts/PERFORMANCE.md).
