import type { Plugin } from 'vite'

export interface CjkFont {
  src: string
  family: string
  faceIndex?: number
  weight?: string
  style?: string
}
export interface CjkFontSplitOptions {
  fonts: CjkFont[]
  cacheDir?: string
  verbose?: boolean
}
export declare function cjkFontSplit(options: CjkFontSplitOptions): Plugin
