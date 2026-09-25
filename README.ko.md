# @xingwangzhe/cjk-font-split-native

[简体中文](README.md) · [English](README.en.md) · [日本語](README.ja.md) · [한국어](README.ko.md)

Vite용 Rust + N-API CJK 글꼴 서브셋 도구입니다. 빌드된 HTML 페이지를 스캔해 페이지별 필요한 문자를 추출하고 WOFF2 서브셋을 생성한 뒤 페이지 범위의 `@font-face`를 삽입합니다. 콘텐츠 주소 기반 캐시는 여러 페이지에서 같은 문자 집합이 필요할 때 결과를 재사용합니다.

이 패키지는 ESM API만 제공합니다. `import`를 사용하세요. CommonJS `require()` 진입점은 내보내지 않습니다. NAPI-RS가 Rust API에서 네이티브 ESM 로더와 TypeScript 선언 파일을 생성합니다.

## Vite 플러그인

```ts
import { defineConfig } from 'vite'
import { cjkFontSplit } from '@xingwangzhe/cjk-font-split-native/vite'

export default defineConfig({
  plugins: [
    cjkFontSplit({
      fonts: [{ src: './src/fonts/LXGWWenKai-Regular.ttf', family: 'LXGW WenKai', weight: '400' }],
      // 선택 사항. 기본값은 Vite의 cacheDir/cjk-font-split-native입니다.
      cacheDir: './node_modules/.vite/cjk-font-split-native',
    }),
  ],
})
```

이 플러그인은 정적 멀티 페이지 빌드를 대상으로 합니다. 각 HTML 페이지와 연결된 스타일시트에서 렌더링 텍스트와 CSS `content` 문자열을 추출하고, 고유한 WOFF2 파일을 `assets/cjk-font-split/`에 출력한 뒤 `@font-face`를 삽입합니다. 클라이언트 렌더링 콘텐츠는 HTML/프리렌더 결과에 텍스트를 포함하거나 저수준 API에 전체 텍스트를 전달하세요. 입력은 TTF, OTF, TTC(`faceIndex`로 글꼴 선택), WOFF, WOFF2를 지원하며 출력은 WOFF2입니다. `family`는 페이지 CSS에서 사용하는 글꼴 패밀리와 일치해야 합니다. `weight`와 `style`은 선택 사항이며 기본값은 각각 `400`, `normal`입니다.

## Native API

```ts
import { readFile } from 'node:fs/promises'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const result = subsetFont(
  await readFile('./LXGWWenKai-Regular.ttf'),
  '유지할 텍스트',
  './.font-cache',
  0, // TTC faceIndex, 선택 사항
)
console.log(result) // { path, hash, cacheHit, bytes, characters }
```

캐시 키는 글꼴 콘텐츠의 BLAKE3, TTC face index, 중복을 제거하고 정렬한 코드 포인트, 서브셋/정규화/압축 알고리즘 버전으로 계산합니다. 캐시 파일 이름은 이 키를 사용합니다. WOFF2 압축 품질은 속도와 크기 벤치마크를 바탕으로 8로 설정했습니다. 추가 전용 `manifest.jsonl`이 키와 WOFF2 파일을 연결합니다. 임시 파일과 원자적 rename으로 기록하며, 동일한 입력은 페이지와 빌드 사이에서 재사용됩니다.

## 개발 및 배포

JS 의존성과 스크립트는 Bun, Vite 플러그인 타입과 선언 생성은 TypeScript 7, Rust 의존성과 빌드는 Cargo로 관리합니다. `bun run ci`는 포맷, lint, TypeScript 컴파일, Rust 테스트와 Clippy, N-API 릴리스 빌드, 통합 테스트 및 벤치마크를 실행합니다. GitHub Actions는 6개 타깃을 빌드하고 테스트합니다. `vX.Y.Z` 형식의 태그를 푸시하면 CI 통과 후 npm Trusted Publishing(OIDC)으로 전체 플랫폼 패키지를 배포합니다. 자세한 내용은 [워크플로](.github/workflows/ci.yml)를 참고하세요.

로컬에서 전체 릴리스를 진행하려면 `gh run download <run-id> --dir artifacts`로 6개 플랫폼의 CI 아티팩트를 다운로드한 다음 `bun run release:local`을 실행하세요. 스크립트가 바이너리를 검증하고 `bun pm pack --dry-run`을 실행한 뒤 Bun으로 배포합니다.

`bun run benchmark`는 네이티브 콜드 캐시, 웜 캐시 적중, 페이지 간 중복 서브셋, 서로 다른 문자 집합, `subset-font` WASM을 비교합니다. 3회 실행의 중앙값과 출력 크기를 보여주며, 결과는 하드웨어에 따라 달라지는 참고 정보입니다.

테스트용 DejaVu Sans 글꼴은 상위 라이선스에 따라 `test/fixtures/`에 있으며 npm 패키지에는 포함되지 않습니다.
