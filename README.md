# @xingwangzhe/cjk-font-split-native

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

面向 Vite 的 Rust + N-API 多语言字体子集工具，重点支持 CJK。它扫描构建页面的可见正文和 CSS 生成内容，提取拉丁字母、重音字符、标点、符号及 CJK 字符，生成 WOFF2 子集并注入页面级 `@font-face`。head 元数据、文本属性、脚本和纯隐藏内容不会进入页面字符集；内容寻址缓存会复用不同页面的相同字符集结果。

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

该插件面向静态多页面构建。它提取每个 HTML 页面可见正文及其关联样式表中的 CSS `content` 字符串，包括页面实际使用的拉丁字母、重音字符、标点、符号和 CJK 字符；head 元数据与 `alt`、`title`、`aria-label` 等属性值不会混入。唯一的 WOFF2 文件输出到 `assets/cjk-font-split/`，并注入 `@font-face`。对于客户端渲染的内容，请将文本纳入 HTML/预渲染结果，或使用底层 API 传入完整文本。支持 TTF、OTF、TTC（通过 `faceIndex` 选择字面）、WOFF、WOFF2 输入，统一输出 WOFF2。`family` 必须与页面使用的 CSS 字体族名称一致；`weight` 和 `style` 可选，默认分别为 `400` 和 `normal`。

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

## 复用字体输入

多页面构建可以使用 `FontSubsetter`：字体归一化和完整字体哈希仅在构造时执行一次，后续调用复用已准备的字体。旧 `subsetFont()` 接口保持兼容，两种接口生成相同的缓存键和 WOFF2 字节。实例拥有字体数据的副本；修改原始 Buffer 不会改变实例。

```ts
import { FontSubsetter } from '@xingwangzhe/cjk-font-split-native'

const prepared = new FontSubsetter(fontBuffer, 0)
const first = prepared.subset('First page 第一页', cacheDir)
const second = prepared.subset('Second page 第二页', cacheDir)
```

`await prepared.subsetAsync(text, cacheDir)` 可将分片工作交给 Node 线程池。字体数据由任务共享，压缩阶段并行，缓存和清单写入保持原子串行；建议最多同时提交 4 个任务。同步接口保持兼容。

### 原生后端与缓存版本

0.4.1 使用内嵌 HarfBuzz 14.6 做字体子集提取，并通过本地 woofwoof 适配层使用 Google Brotli 1.2 编码 WOFF2。Brotli 质量仍为 8，未关闭字体变换或丢弃字形信息。`FontSubsetter` 预处理字体后复用子集加速数据；同一字体的子集提取通过短锁保护，压缩在线程池中并行执行。输入先解码为 SFNT（WOFF/WOFF2）或提取集合中的字体（TTC），再规范化为可复用的 HarfBuzz 预处理字体；保留原有 TrueType/CFF 轮廓，不进行有损曲线转换。没有系统 HarfBuzz/Brotli 安装要求。

算法指纹已更新，因此旧字体缓存不会被当作新输出使用。0.3.x 与 0.4.1 的字体文件名及 WOFF2 字节会不同；同一 0.4.1 版本内的同步、预处理和异步接口仍产生相同结果。HarfBuzz 会正确重建垂直度量表，避免原后端多余的 `vmtx` 数据。新版也会保留双向排版所需的镜像符号闭包，因此实际 cmap 可能包含请求字符以外的对应镜像符号。第三方许可证随 npm 包附带。
