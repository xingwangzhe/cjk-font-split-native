# @xingwangzhe/cjk-font-split-native

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Vite 向けの Rust + N-API 製 CJK フォントサブセットツールです。ビルド後の HTML ページを走査し、ページごとに必要な文字を抽出して WOFF2 サブセットを生成し、ページ単位の `@font-face` を挿入します。コンテンツアドレス方式のキャッシュにより、異なるページで同じ文字セットが必要な場合に結果を再利用します。

本パッケージの API は ESM のみです。`import` を使用してください。CommonJS の `require()` エントリーポイントは公開していません。NAPI-RS が Rust API からネイティブ ESM ローダーと TypeScript 宣言を生成します。

## Vite プラグイン

```ts
import { defineConfig } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

export default defineConfig({
  plugins: [
    cjkFontSplit({
      fonts: [{ src: './src/fonts/LXGWWenKai-Regular.ttf', family: 'LXGW WenKai', weight: '400' }],
      // 任意。既定値は Vite の cacheDir/cjk-font-split-native です。
      cacheDir: './node_modules/.vite/cjk-font-split-native',
    }),
  ],
})
```

このプラグインは静的なマルチページビルドを対象とします。各 HTML ページとリンクされたスタイルシートから表示テキストおよび CSS `content` の文字列を抽出し、重複のない WOFF2 ファイルを `assets/cjk-font-split/` に出力して、`@font-face` を挿入します。クライアント側で描画されるコンテンツの場合は、HTML/プリレンダリング結果にテキストを含めるか、低レベル API に完全なテキストを渡してください。入力形式は TTF、OTF、TTC（`faceIndex` で書体を選択）、WOFF、WOFF2 に対応し、出力は WOFF2 です。`family` はページで使う CSS のフォントファミリー名と一致させてください。`weight` と `style` は省略可能で、既定値はそれぞれ `400` と `normal` です。

## Native API

```ts
import { readFile } from 'node:fs/promises'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const result = subsetFont(
  await readFile('./LXGWWenKai-Regular.ttf'),
  '残すテキスト',
  './.font-cache',
  0, // TTC faceIndex（省略可能）
)
console.log(result) // { path, hash, cacheHit, bytes, characters }
```

キャッシュキーはフォント内容の BLAKE3、TTC の face index、重複を除いてソートしたコードポイント、およびサブセット化・正規化・圧縮アルゴリズムのバージョンから計算されます。キャッシュファイル名にはこのキーを使用します。WOFF2 の圧縮品質は速度とサイズのベンチマークをもとに 8 に設定しています。追記型の `manifest.jsonl` がキーと WOFF2 ファイルを対応付けます。一時ファイルとアトミックな rename を使って書き込み、同じ入力をページ間・ビルド間で再利用します。

## 開発とリリース

JS の依存関係とスクリプトには Bun、Vite プラグインの型と宣言生成には TypeScript 7、Rust の依存関係とビルドには Cargo を使用します。`bun run ci` はフォーマット、lint、TypeScript コンパイル、Rust テストと Clippy、N-API リリースビルド、統合テスト、ベンチマークを実行します。GitHub Actions は 6 つのターゲットをビルド・テストします。`vX.Y.Z` 形式のタグを push すると、CI 成功後に npm Trusted Publishing（OIDC）で全プラットフォーム対応パッケージを公開します。詳しくは[ワークフロー](.github/workflows/ci.yml)を参照してください。

ローカルで完全なリリースを行うには、`gh run download <run-id> --dir artifacts` で CI の 6 プラットフォーム分の成果物を取得し、`bun run release:local` を実行します。スクリプトはバイナリを検証し、`bun pm pack --dry-run` を実行した後、Bun で公開します。

`bun run benchmark` では、ネイティブのコールドキャッシュ、ウォームキャッシュ、ページ間での重複、異なる文字セット、および `subset-font` WASM を比較します。3 回の実行の中央値と出力サイズを表示します。結果はハードウェアに依存するため、参考値として扱ってください。

テスト用の DejaVu Sans フォントは上流ライセンスに従って `test/fixtures/` に置かれており、npm パッケージには含まれません。
