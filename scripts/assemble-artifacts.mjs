import { access, copyFile, readdir } from 'node:fs/promises'
import path from 'node:path'

const root = process.cwd()
const input = path.resolve(root, process.argv[2] ?? 'artifacts')
const expected = [
  'cjk-font-split-native.darwin-x64.node',
  'cjk-font-split-native.darwin-arm64.node',
  'cjk-font-split-native.win32-x64-msvc.node',
  'cjk-font-split-native.linux-x64-gnu.node',
  'cjk-font-split-native.linux-x64-musl.node',
  'cjk-font-split-native.linux-arm64-gnu.node',
]

async function findFiles(dir) {
  const found = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const file = path.join(dir, entry.name)
    if (entry.isDirectory()) found.push(...(await findFiles(file)))
    else if (entry.isFile() && entry.name.endsWith('.node')) found.push(file)
  }
  return found
}

let files = []
try {
  files = await findFiles(input)
} catch (error) {
  if (error.code !== 'ENOENT') throw error
}
const sources = new Map()
for (const file of files) sources.set(path.basename(file), file)
for (const name of expected) {
  const source = sources.get(name) ?? path.join(root, name)
  try {
    await access(source)
  } catch {
    throw new Error(`Missing ${name}; download all six CI artifacts into ${input} before local publish.`)
  }
  if (path.resolve(source) !== path.resolve(root, name)) await copyFile(source, path.join(root, name))
}
console.info(`Prepared ${expected.length} native bindings for the npm package.`)
