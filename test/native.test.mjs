import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import test from 'node:test'
import { deflateSync } from 'node:zlib'
import { subsetFont } from '@xingwangzhe/cjk-font-split-native'

const here = path.dirname(fileURLToPath(import.meta.url))
const font = await readFile(path.join(here, 'fixtures/DejaVuSans.ttf'))

const woff1 = (sfnt) => {
  const count = sfnt.readUInt16BE(4)
  const header = Buffer.alloc(44)
  const records = Buffer.alloc(count * 20)
  const payloads = []
  header.write('wOFF', 0, 'ascii')
  sfnt.copy(header, 4, 0, 4)
  header.writeUInt16BE(count, 12)
  header.writeUInt32BE(sfnt.length, 16)
  let dataOffset = header.length + records.length
  for (let index = 0; index < count; index++) {
    const sourceRecord = 12 + index * 16
    const tag = sfnt.subarray(sourceRecord, sourceRecord + 4)
    const offset = sfnt.readUInt32BE(sourceRecord + 8)
    const length = sfnt.readUInt32BE(sourceRecord + 12)
    const table = sfnt.subarray(offset, offset + length)
    const compressed = deflateSync(table)
    const payload = compressed.length < table.length ? compressed : table
    const record = index * 20
    records.write(tag.toString('ascii'), record, 'ascii')
    records.writeUInt32BE(dataOffset, record + 4)
    records.writeUInt32BE(payload.length, record + 8)
    records.writeUInt32BE(table.length, record + 12)
    records.writeUInt32BE(sfnt.readUInt32BE(sourceRecord + 4), record + 16)
    payloads.push(payload)
    dataOffset += payload.length
    const padding = (4 - (dataOffset % 4)) % 4
    if (padding) payloads.push(Buffer.alloc(padding))
    dataOffset += padding
  }
  const result = Buffer.concat([header, records, ...payloads])
  result.writeUInt32BE(result.length, 8)
  return result
}

test('subsets font and reuses content-addressed cache independent of input ordering', async () => {
  const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-split-'))
  try {
    const first = subsetFont(font, 'Hello, 世界', cache)
    const second = subsetFont(font, '界世 ,olleH', cache)
    assert.equal(first.cacheHit, false)
    assert.equal(second.cacheHit, true)
    assert.equal(first.hash, second.hash)
    assert.ok(first.bytes > 0)
    assert.equal(first.characters, new Set('Hello, 世界').size)
    const woff2 = await readFile(first.path)
    assert.equal(woff2.toString('ascii', 0, 4), 'wOF2')
    const manifest = await readFile(path.join(cache, 'manifest.jsonl'), 'utf8')
    assert.match(manifest, new RegExp(first.hash))
  } finally {
    await rm(cache, { recursive: true, force: true })
  }
})

test('rejects empty text and invalid font bytes', async () => {
  const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-split-'))
  try {
    assert.throws(() => subsetFont(font, '', cache), /at least one character/)
    assert.throws(() => subsetFont(Buffer.from('invalid'), 'abc', cache), /unsupported font/)
  } finally {
    await rm(cache, { recursive: true, force: true })
  }
})

test('accepts WOFF2 as input as well as output', async () => {
  const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-split-'))
  try {
    const first = subsetFont(font, 'WOFF2 round trip', cache)
    const second = subsetFont(await readFile(first.path), 'WOFF2 subset', cache)
    assert.equal(second.cacheHit, false)
    assert.ok(second.bytes > 0)
    assert.equal((await readFile(second.path)).toString('ascii', 0, 4), 'wOF2')
  } finally {
    await rm(cache, { recursive: true, force: true })
  }
})

test('accepts WOFF1 as input', async () => {
  const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-woff1-'))
  try {
    const result = subsetFont(woff1(font), 'WOFF1 input', cache)
    assert.ok(result.bytes > 0)
    assert.equal((await readFile(result.path)).toString('ascii', 0, 4), 'wOF2')
  } finally {
    await rm(cache, { recursive: true, force: true })
  }
})

const ttcPath = process.env.CJK_TEST_TTC
test('subsets a selected TTC face', { skip: !ttcPath }, async () => {
  const cache = await mkdtemp(path.join(os.tmpdir(), 'cjk-ttc-'))
  try {
    const result = subsetFont(await readFile(ttcPath), '中文', cache, 1)
    assert.ok(result.bytes > 0)
    assert.equal((await readFile(result.path)).toString('ascii', 0, 4), 'wOF2')
  } finally {
    await rm(cache, { recursive: true, force: true })
  }
})
