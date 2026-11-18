declare module 'subset-font' {
  interface SubsetFontOptions {
    targetFormat?: 'woff2' | 'woff' | 'truetype' | 'opentype'
  }

  export default function subsetFont(
    font: Buffer,
    text: string,
    options?: SubsetFontOptions,
  ): Promise<Buffer>
}
