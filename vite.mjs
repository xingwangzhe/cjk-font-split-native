import { readFile, readdir, mkdir, copyFile, writeFile } from 'node:fs/promises'
import path from 'node:path'
import native from './index.js'

const { subsetFont } = native

const htmlFiles = async (dir) => {
  const found = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) found.push(...(await htmlFiles(full)))
    else if (entry.isFile() && entry.name.endsWith('.html')) found.push(full)
  }
  return found
}

const decodeEntities = (value) =>
  value
    .replace(/&#(x[\da-f]+|\d+);/gi, (_, code) =>
      String.fromCodePoint(code[0].toLowerCase() === 'x' ? parseInt(code.slice(1), 16) : Number(code)),
    )
    .replace(
      /&(?:nbsp|amp|lt|gt|quot|apos);/g,
      (entity) => ({ '&nbsp;': ' ', '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&apos;': "'" })[entity],
    )

const cssContent = (css) =>
  [...css.matchAll(/\bcontent\s*:\s*(["'])(.*?)\1\s*(?:!important\s*)?(?:;|})/gis)]
    .map(([, , value]) =>
      value.replace(/\\([\da-f]{1,6})\s?|\\(.)/gi, (_, hex, char) =>
        hex ? String.fromCodePoint(parseInt(hex, 16)) : char,
      ),
    )
    .join(' ')

const visibleText = (html) => {
  const inlineCssText = [...html.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style\s*>/gi)]
    .map(([, css]) => cssContent(css))
    .join(' ')
  const markupText = html
    .replace(/<(script|style|noscript|template)\b[^>]*>[\s\S]*?<\/\1\s*>/gi, ' ')
    .replace(/<!--[\s\S]*?-->/g, ' ')
    .replace(/\b(?:alt|title|aria-label)\s*=\s*(["'])(.*?)\1/gi, ' $2 ')
    .replace(/<[^>]+>/g, ' ')
  return `${inlineCssText} ${decodeEntities(markupText)}`
}

const attribute = (tag, name) => tag.match(new RegExp(`\\b${name}\\s*=\\s*(["'])(.*?)\\1`, 'i'))?.[2]

const cssString = (value) => `"${value.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`

/**
 * Create per-HTML-page WOFF2 subsets and inject page-scoped @font-face rules.
 * This deliberately runs after Vite has emitted all pages and assets.
 */
export function cjkFontSplit(options) {
  if (!options?.fonts?.length) throw new TypeError('cjkFontSplit requires fonts: [{ src, family }]')
  let root = process.cwd()
  let outputDir = path.resolve(root, 'dist')
  let cacheDir
  return {
    name: 'cjk-font-split-native',
    apply: 'build',
    configResolved(config) {
      root = config.root
      outputDir = path.resolve(root, config.build.outDir)
      cacheDir = path.resolve(root, options.cacheDir ?? path.join(config.cacheDir, 'cjk-font-split-native'))
    },
    async closeBundle() {
      const dir = outputDir
      const pages = await htmlFiles(dir)
      const fontBytes = await Promise.all(
        options.fonts.map(async (font) => ({
          ...font,
          bytes: await readFile(path.resolve(root, font.src)),
        })),
      )
      const emitted = new Set()
      const cssCache = new Map()
      for (const page of pages) {
        const html = await readFile(page, 'utf8')
        const linkedCss = []
        for (const [tag] of html.matchAll(/<link\b[^>]*>/gi)) {
          if (!attribute(tag, 'rel')?.split(/\s+/).includes('stylesheet')) continue
          const href = attribute(tag, 'href')
          if (!href || /^(?:[a-z]+:|\/\/|data:)/i.test(href)) continue
          const cssPath = path.resolve(href.startsWith('/') ? dir : path.dirname(page), href.replace(/^\/+/, ''))
          if (!cssPath.startsWith(`${dir}${path.sep}`)) continue
          if (!cssCache.has(cssPath)) {
            try {
              cssCache.set(cssPath, cssContent(await readFile(cssPath, 'utf8')))
            } catch {
              cssCache.set(cssPath, '')
            }
          }
          linkedCss.push(cssCache.get(cssPath))
        }
        const text = `${visibleText(html)} ${linkedCss.join(' ')}`
        if (!text.trim()) continue
        const rules = []
        for (const font of fontBytes) {
          const result = subsetFont(font.bytes, text, cacheDir, font.faceIndex ?? 0)
          const filename = `${result.hash}.woff2`
          const target = path.join(dir, 'assets', 'cjk-font-split', filename)
          if (!emitted.has(filename)) {
            await mkdir(path.dirname(target), { recursive: true })
            await copyFile(result.path, target)
            emitted.add(filename)
          }
          const relative = path.relative(path.dirname(page), target).split(path.sep).join('/')
          const url = relative.startsWith('.') ? relative : `./${relative}`
          rules.push(
            `@font-face{font-family:${cssString(font.family)};font-style:${font.style ?? 'normal'};font-weight:${font.weight ?? '400'};font-display:swap;src:url(${cssString(url)}) format("woff2")}`,
          )
        }
        const injected = `<style data-cjk-font-split>${rules.join('')}</style>`
        const updated = /<\/head\s*>/i.test(html)
          ? html.replace(/<\/head\s*>/i, `${injected}</head>`)
          : `${injected}${html}`
        await writeFile(page, updated)
      }
      if (options.verbose) {
        console.info(
          `[cjk-font-split-native] processed ${pages.length} HTML pages; emitted ${emitted.size} unique subsets`,
        )
      }
    },
  }
}
