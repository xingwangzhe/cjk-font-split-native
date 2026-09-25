import type { Plugin } from 'vite'
import { cjkFontSplit, type CjkFontSplitOptions } from '@xingwangzhe/cjk-font-split-native/vite'

const options = {
  fonts: [{ src: './fonts/example.ttf', family: 'Example', weight: 400, faceIndex: 0 }],
  cacheDir: './.cache/fonts',
} satisfies CjkFontSplitOptions

const plugin: Plugin = cjkFontSplit(options)
void plugin
