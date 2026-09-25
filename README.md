# @xingwangzhe/cjk-font-split-native

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

面向 Vite 的 Rust + N-API CJK 字体子集工具。它扫描构建生成的 HTML 页面，为每个页面提取所需字符，生成 WOFF2 子集并注入页面级 `@font-face`。内容寻址缓存会复用不同页面的相同字符集结果。

本包仅提供 ESM API。请使用 `import`；包不导出 CommonJS `require()` 入口。NAPI-RS 根据 Rust API 生成原生 ESM 加载器和 TypeScript 声明。

## Vite 插件

```ts
import { defineConfig } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

export default defineConfig({
  plugins: [
    cjkFontSplit({
      fonts: [{ src: './src/fonts/LXGWWenKai-Regular.ttf', family: 'LXGW WenKai', weight: '400' }],
      // 可选；默认使用 Vite 的 cacheDir/cjk-font-split-native。
      cacheDir: './node_modules/.vite/cjk-font-split-native',
    }),
  ],
})
```

该插件面向静态多页面构建。它提取每个 HTML 页面及其关联样式表中的渲染文本和 CSS `content` 字符串，将唯一的 WOFF2 文件输出到 `assets/cjk-font-split/`，并注入 `@font-face`。对于客户端渲染的内容，请将文本纳入 HTML/预渲染结果，或使用底层 API 传入完整文本。支持 TTF、OTF、TTC（通过 `faceIndex` 选择字面）、WOFF、WOFF2 输入，统一输出 WOFF2。`family` 必须与页面使用的 CSS 字体族名称一致；`weight` 和 `style` 可选，默认分别为 `400` 和 `normal`。

## Native API

```ts
import { readFile } from 'node:fs/promises'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const result = subsetFont(
  await readFile('./LXGWWenKai-Regular.ttf'),
  '需要保留的文字',
  './.font-cache',
  0, // TTC faceIndex，可选
)
console.log(result) // { path, hash, cacheHit, bytes, characters }
```

缓存键由字体内容 BLAKE3、TTC 字面索引、排序去重后的码点以及子集化/规范化/压缩算法版本计算。缓存文件采用该键命名，WOFF2 压缩质量为 8（根据速度和体积基准选择）。追加写入的 `manifest.jsonl` 保存键到 WOFF2 文件的映射。缓存写入使用临时文件和原子重命名；相同输入会跨页面和构建复用结果。

## 开发与发布

使用 Bun 管理 JS 依赖和脚本，TypeScript 7 为 Vite 插件提供类型并生成声明，Cargo 管理 Rust 依赖和构建。`bun run ci` 执行格式检查、lint、TypeScript 编译、Rust 测试与 Clippy、N-API 发布构建、集成测试和基准测试。GitHub Actions 构建并测试六个目标平台；推送匹配的 `vX.Y.Z` 标签后，工作流会在 CI 通过后使用 npm Trusted Publishing（OIDC）发布完整包，详见 [工作流](.github/workflows/ci.yml)。

本地完整发布时，可用 `gh run download <run-id> --dir artifacts` 下载六个平台的 CI 产物，再运行 `bun run release:local`。该脚本会校验二进制、执行 `bun pm pack --dry-run`，并通过 Bun 发布。

运行 `bun run benchmark` 可对比冷缓存原生分片、热缓存命中、跨页面重复分片、不同字符集以及 `subset-font` WASM。基准执行三次并报告中位耗时和输出体积；结果会受硬件影响，仅供参考。

测试字体 DejaVu Sans 位于 `test/fixtures/`，遵循上游许可，且不会被打包进 npm 发布内容。
