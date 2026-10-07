#![deny(clippy::all)]

use flate2::read::ZlibDecoder;
use napi::bindgen_prelude::{AsyncTask, Buffer};
use napi::{Env, Error, Result, Status, Task};
use napi_derive::napi;
use serde::Serialize;
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::fs::{self, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

const ALGORITHM_VERSION: &str = "harfbuzz-14.6;woff-normalizer-1;google-brotli-1.2-woff2-q8";
const WOFF2_QUALITY: usize = 8;
static CACHE_LOCKS: LazyLock<[Mutex<()>; 256]> =
  LazyLock::new(|| std::array::from_fn(|_| Mutex::new(())));
// Bound synchronization memory while coalescing identical cold requests.
static SUBSET_LOCKS: LazyLock<[Mutex<()>; 256]> =
  LazyLock::new(|| std::array::from_fn(|_| Mutex::new(())));

#[napi(object)]
pub struct SubsetResult {
  pub path: String,
  pub hash: String,
  pub cache_hit: bool,
  pub bytes: u32,
  pub characters: u32,
}

#[derive(Serialize)]
struct ManifestEntry {
  path: String,
  bytes: usize,
  characters: usize,
  algorithm: &'static str,
}

#[napi]
pub fn subset_font(
  font: Buffer,
  text: String,
  cache_dir: String,
  face_index: Option<u32>,
) -> Result<SubsetResult> {
  let face = face_index.unwrap_or(0);
  subset_cached(font_hasher(&font, face), text, cache_dir, |chars| {
    let normalized = normalize_font(&font, face).map_err(|e| Error::new(Status::InvalidArg, e))?;
    subset_to_woff2(&normalized, chars)
  })
}

/// Reuse a validated font and its BLAKE3 prefix across a multi-page build.
#[napi]
pub struct FontSubsetter {
  prepared: Arc<PreparedFont>,
  prefix: blake3::Hasher,
}

#[napi]
impl FontSubsetter {
  #[napi(constructor)]
  pub fn new(font: Buffer, face_index: Option<u32>) -> Result<Self> {
    let face = face_index.unwrap_or(0);
    let prefix = font_hasher(&font, face);
    let normalized = normalize_font(&font, face).map_err(|e| Error::new(Status::InvalidArg, e))?;
    Ok(Self {
      prepared: Arc::new(PreparedFont::new(normalized)?),
      prefix,
    })
  }

  #[napi(ts_return_type = "Promise<SubsetResult>")]
  pub fn subset_async(&self, text: String, cache_dir: String) -> AsyncTask<SubsetTask> {
    AsyncTask::new(SubsetTask {
      prepared: Arc::clone(&self.prepared),
      prefix: self.prefix.clone(),
      text,
      cache_dir,
    })
  }

  #[napi]
  pub fn subset(&self, text: String, cache_dir: String) -> Result<SubsetResult> {
    subset_cached(self.prefix.clone(), text, cache_dir, |chars| {
      self.prepared.subset(chars)
    })
  }
}

pub struct SubsetTask {
  prepared: Arc<PreparedFont>,
  prefix: blake3::Hasher,
  text: String,
  cache_dir: String,
}

impl Task for SubsetTask {
  type Output = SubsetResult;
  type JsValue = SubsetResult;

  fn compute(&mut self) -> Result<Self::Output> {
    subset_cached(
      self.prefix.clone(),
      std::mem::take(&mut self.text),
      std::mem::take(&mut self.cache_dir),
      |chars| self.prepared.subset(chars),
    )
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output)
  }
}

fn font_hasher(font: &[u8], face: u32) -> blake3::Hasher {
  let mut hasher = blake3::Hasher::new();
  hasher.update(ALGORITHM_VERSION.as_bytes());
  hasher.update(&face.to_le_bytes());
  hasher.update(&(font.len() as u64).to_le_bytes());
  hasher.update(font);
  hasher
}

fn subset_cached(
  mut hasher: blake3::Hasher,
  text: String,
  cache_dir: String,
  render: impl FnOnce(&HashSet<char>) -> Result<Vec<u8>>,
) -> Result<SubsetResult> {
  let chars: HashSet<char> = text.chars().collect();
  if chars.is_empty() {
    return Err(Error::new(
      Status::InvalidArg,
      "text must contain at least one character",
    ));
  }
  let mut sorted: Vec<char> = chars.iter().copied().collect();
  sorted.sort_unstable();
  // Feed the same canonical bytes in one update instead of one call per scalar.
  let mut codepoints = Vec::with_capacity(sorted.len() * 4);
  for ch in &sorted {
    codepoints.extend_from_slice(&(*ch as u32).to_le_bytes());
  }
  hasher.update(&codepoints);
  let digest = hasher.finalize();
  let key = digest.to_hex().to_string();
  let cache = PathBuf::from(cache_dir);
  let output = cache.join(format!("{key}.woff2"));

  if let Some(metadata) = fs::metadata(&output).ok().filter(|value| value.is_file()) {
    let bytes = metadata.len() as u32;
    return Ok(SubsetResult {
      path: output.to_string_lossy().into_owned(),
      hash: key,
      cache_hit: true,
      bytes,
      characters: sorted.len() as u32,
    });
  }

  let mut cache_hasher = DefaultHasher::new();
  cache.hash(&mut cache_hasher);
  let cache_shard = cache_hasher.finish() as u8;
  // Include the destination: independent cache directories should not block
  // one another merely because their requested glyph sets are identical.
  let subset_shard = cache_shard ^ digest.as_bytes()[0];
  let _subset_guard = SUBSET_LOCKS[subset_shard as usize]
    .lock()
    .map_err(|_| Error::new(Status::GenericFailure, "subset lock poisoned"))?;
  // Another worker may have completed this exact subset while we waited.
  if let Some(metadata) = fs::metadata(&output).ok().filter(|value| value.is_file()) {
    return Ok(SubsetResult {
      path: output.to_string_lossy().into_owned(),
      hash: key,
      cache_hit: true,
      bytes: metadata.len() as u32,
      characters: sorted.len() as u32,
    });
  }
  fs::create_dir_all(&cache).map_err(io_error)?;
  let woff2 = render(&chars)?;
  let _guard = CACHE_LOCKS[cache_shard as usize]
    .lock()
    .map_err(|_| Error::new(Status::GenericFailure, "cache lock poisoned"))?;
  if output.is_file() {
    return Ok(SubsetResult {
      path: output.to_string_lossy().into_owned(),
      hash: key,
      cache_hit: true,
      bytes: fs::metadata(&output).map_err(io_error)?.len() as u32,
      characters: sorted.len() as u32,
    });
  }
  atomic_write(&output, &woff2).map_err(io_error)?;
  update_manifest(&cache, &key, &output, woff2.len(), sorted.len()).map_err(io_error)?;
  Ok(SubsetResult {
    path: output.to_string_lossy().into_owned(),
    hash: key,
    cache_hit: false,
    bytes: woff2.len() as u32,
    characters: sorted.len() as u32,
  })
}

// The borrowed HarfBuzz blob points into `data`. Field order drops the face
// before its backing bytes; all users retain an Arc for the full operation.
struct PreparedFont {
  face: Mutex<PreparedFace>,
  _data: Arc<Vec<u8>>,
}
struct PreparedFace(hb_subset::PreprocessedFontFace<'static>);

// HarfBuzz faces are reference-counted and not thread-affine. We only move the
// handle across workers; the mutex serializes every operation on this face.
unsafe impl Send for PreparedFace {}

impl PreparedFont {
  fn new(data: Vec<u8>) -> Result<Self> {
    let data = Arc::new(data);
    let blob = hb_subset::Blob::from_bytes(&data).map_err(hb_error)?;
    let face = hb_subset::FontFace::new(blob).map_err(hb_error)?;
    let prepared = face.preprocess_for_subsetting();
    // `data` is immutable, owns the allocation, and outlives the face above.
    let prepared = unsafe {
      std::mem::transmute::<
        hb_subset::PreprocessedFontFace<'_>,
        hb_subset::PreprocessedFontFace<'static>,
      >(prepared)
    };
    drop(face);
    Ok(Self {
      face: Mutex::new(PreparedFace(prepared)),
      _data: data,
    })
  }
  fn subset(&self, chars: &HashSet<char>) -> Result<Vec<u8>> {
    let subset = {
      let prepared = self
        .face
        .lock()
        .map_err(|_| Error::new(Status::GenericFailure, "font lock poisoned"))?;
      subset_sfnt(&prepared.0, chars)?
    };
    compress_subset(&subset)
  }
}

fn hb_error(error: impl std::fmt::Display) -> Error {
  Error::new(
    Status::GenericFailure,
    format!("HarfBuzz subset failed: {error}"),
  )
}
fn subset_sfnt(face: &hb_subset::FontFace<'_>, chars: &HashSet<char>) -> Result<Vec<u8>> {
  let mut input = hb_subset::SubsetInput::new().map_err(hb_error)?;
  {
    let mut unicodes = input.unicode_set();
    for ch in chars {
      unicodes.insert(*ch);
    }
  }
  let subset = input.subset_font(face).map_err(hb_error)?;
  let bytes = subset.underlying_blob().to_vec();
  Ok(bytes)
}
fn compress_subset(subset: &[u8]) -> Result<Vec<u8>> {
  woofwoof::compress(subset, "", WOFF2_QUALITY, true)
    .ok_or_else(|| Error::new(Status::GenericFailure, "WOFF2 compression failed"))
}
fn subset_to_woff2(font: &[u8], chars: &HashSet<char>) -> Result<Vec<u8>> {
  let blob = hb_subset::Blob::from_bytes(font).map_err(hb_error)?;
  let face = hb_subset::FontFace::new(blob).map_err(hb_error)?;
  compress_subset(&subset_sfnt(&face, chars)?)
}

fn io_error(error: std::io::Error) -> Error {
  Error::new(Status::GenericFailure, error.to_string())
}

fn read_u16(data: &[u8], at: usize) -> std::result::Result<u16, String> {
  let slice = data.get(at..at + 2).ok_or("font data is truncated")?;
  Ok(u16::from_be_bytes([slice[0], slice[1]]))
}

fn read_u32(data: &[u8], at: usize) -> std::result::Result<u32, String> {
  let slice = data.get(at..at + 4).ok_or("font data is truncated")?;
  Ok(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn normalize_font(data: &[u8], face: u32) -> std::result::Result<Vec<u8>, String> {
  match data.get(..4) {
    Some(b"wOF2") => woofwoof::decompress(data)
      .ok_or_else(|| "WOFF2 decode failed".to_string())
      .or_else(|_| normalize_woff2_allsorts(data))
      .map_err(|e| format!("WOFF2 decode failed: {e}")),
    Some(b"wOFF") => {
      if face != 0 {
        return Err("faceIndex is only valid for TTC collections".into());
      }
      normalize_woff(data)
    }
    Some(b"ttcf") => normalize_ttc(data, face),
    Some(b"\0\x01\0\0") | Some(b"OTTO") | Some(b"true") => {
      if face != 0 {
        return Err("faceIndex is only valid for TTC collections".into());
      }
      Ok(data.to_vec())
    }
    _ => Err("unsupported font; expected TTF, OTF, TTC, WOFF, or WOFF2".into()),
  }
}

fn normalize_woff2_allsorts(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
  use allsorts::binary::read::ReadScope;
  use allsorts::font_data::FontData;
  use allsorts::tables::{FontTableProvider, SfntVersion};

  let font = ReadScope::new(data)
    .read::<FontData>()
    .map_err(|e| format!("allsorts parse: {e:?}"))?;
  let provider = font
    .table_provider(0)
    .map_err(|e| format!("allsorts table provider: {e:?}"))?;
  let tables = provider
    .table_tags()
    .ok_or("WOFF2 has no table directory")?
    .into_iter()
    .map(|tag| {
      let data = provider
        .read_table_data(tag)
        .map_err(|e| format!("allsorts table decode: {e:?}"))?;
      Ok((tag.to_be_bytes().to_vec(), data.into_owned()))
    })
    .collect::<std::result::Result<Vec<_>, String>>()?;
  build_sfnt(provider.sfnt_version(), tables)
}

fn normalize_woff(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
  let flavor = read_u32(data, 4)?;
  let count = read_u16(data, 12)? as usize;
  let mut tables = Vec::with_capacity(count);
  let mut at = 44;
  for _ in 0..count {
    let tag = data
      .get(at..at + 4)
      .ok_or("truncated WOFF table directory")?
      .to_vec();
    let offset = read_u32(data, at + 4)? as usize;
    let compressed = read_u32(data, at + 8)? as usize;
    let original = read_u32(data, at + 12)? as usize;
    let table_data = data
      .get(offset..offset + compressed)
      .ok_or("truncated WOFF table")?;
    let decoded = if compressed == original {
      table_data.to_vec()
    } else {
      let mut decoder = ZlibDecoder::new(table_data);
      let mut out = Vec::with_capacity(original);
      decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("WOFF inflate failed: {e}"))?;
      if out.len() != original {
        return Err("WOFF table length mismatch".into());
      }
      out
    };
    tables.push((tag, decoded));
    at += 20;
  }
  build_sfnt(flavor, tables)
}

fn normalize_ttc(data: &[u8], face: u32) -> std::result::Result<Vec<u8>, String> {
  let count = read_u32(data, 8)? as usize;
  if face as usize >= count {
    return Err(format!(
      "TTC faceIndex {face} out of range (faces: {count})"
    ));
  }
  let face_offset = read_u32(data, 12 + face as usize * 4)? as usize;
  let flavor = read_u32(data, face_offset)?;
  let table_count = read_u16(data, face_offset + 4)? as usize;
  let mut tables = Vec::with_capacity(table_count);
  for i in 0..table_count {
    let at = face_offset + 12 + i * 16;
    let tag = data
      .get(at..at + 4)
      .ok_or("truncated TTC table directory")?
      .to_vec();
    let offset = read_u32(data, at + 8)? as usize;
    let length = read_u32(data, at + 12)? as usize;
    tables.push((
      tag,
      data
        .get(offset..offset + length)
        .ok_or("truncated TTC table")?
        .to_vec(),
    ));
  }
  build_sfnt(flavor, tables)
}

fn checksum(bytes: &[u8]) -> u32 {
  bytes
    .chunks(4)
    .map(|chunk| {
      let mut word = [0u8; 4];
      word[..chunk.len()].copy_from_slice(chunk);
      u32::from_be_bytes(word)
    })
    .fold(0u32, u32::wrapping_add)
}

fn build_sfnt(
  flavor: u32,
  mut tables: Vec<(Vec<u8>, Vec<u8>)>,
) -> std::result::Result<Vec<u8>, String> {
  tables.sort_by(|a, b| a.0.cmp(&b.0));
  let count = u16::try_from(tables.len()).map_err(|_| "too many sfnt tables")?;
  let mut power = 1u16;
  let mut selector = 0u16;
  while power.saturating_mul(2) <= count {
    power *= 2;
    selector += 1;
  }
  let mut out = Vec::new();
  out.extend_from_slice(&flavor.to_be_bytes());
  out.extend_from_slice(&count.to_be_bytes());
  out.extend_from_slice(&(power * 16).to_be_bytes());
  out.extend_from_slice(&selector.to_be_bytes());
  out.extend_from_slice(&(count * 16 - power * 16).to_be_bytes());
  let directory_start = out.len();
  out.resize(directory_start + tables.len() * 16, 0);
  for (index, (tag, bytes)) in tables.iter().enumerate() {
    if tag.len() != 4 {
      return Err("invalid sfnt table tag".into());
    }
    let record = directory_start + index * 16;
    out[record..record + 4].copy_from_slice(tag);
    out[record + 4..record + 8].copy_from_slice(&checksum(bytes).to_be_bytes());
    let offset = u32::try_from(out.len()).map_err(|_| "font is too large")?;
    out[record + 8..record + 12].copy_from_slice(&offset.to_be_bytes());
    let length = u32::try_from(bytes.len()).map_err(|_| "font table is too large")?;
    out[record + 12..record + 16].copy_from_slice(&length.to_be_bytes());
    out.extend_from_slice(bytes);
    while out.len() % 4 != 0 {
      out.push(0);
    }
  }
  Ok(out)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
  let temp = path.with_extension(format!("tmp-{}", std::process::id()));
  let mut file = OpenOptions::new()
    .write(true)
    .create_new(true)
    .open(&temp)?;
  file.write_all(bytes)?;
  file.sync_all()?;
  match fs::rename(&temp, path) {
    Ok(()) => Ok(()),
    Err(_e) if path.exists() => {
      let _ = fs::remove_file(temp);
      Ok(())
    }
    Err(e) => {
      let _ = fs::remove_file(temp);
      Err(e)
    }
  }
}

fn update_manifest(
  cache: &Path,
  key: &str,
  output: &Path,
  bytes: usize,
  characters: usize,
) -> std::io::Result<()> {
  let manifest_path = cache.join("manifest.jsonl");
  let mut file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(manifest_path)?;
  let mut entry = serde_json::to_vec(&serde_json::json!({ "key": key, "entry": ManifestEntry {
      path: output.file_name().unwrap_or_default().to_string_lossy().into_owned(),
      bytes,
      characters,
      algorithm: ALGORITHM_VERSION,
  }}))
  .expect("serializable manifest entry");
  entry.push(b'\n');
  file.write_all(&entry)?;
  file.sync_data()
}

#[cfg(test)]
mod tests {
  use super::*;
  use flate2::write::ZlibEncoder;
  use flate2::Compression;

  fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    for i in 0..read_u16(font, 4).unwrap() as usize {
      let rec = 12 + i * 16;
      if &font[rec..rec + 4] == tag {
        let offset = read_u32(font, rec + 8).unwrap() as usize;
        let len = read_u32(font, rec + 12).unwrap() as usize;
        return &font[offset..offset + len];
      }
    }
    panic!("missing table");
  }
  fn metric(font: &[u8], glyph: usize, vertical: bool) -> (u16, i16) {
    let header = table(font, if vertical { b"vhea" } else { b"hhea" });
    let metrics = table(font, if vertical { b"vmtx" } else { b"hmtx" });
    let count = read_u16(header, 34).unwrap() as usize;
    let advance = read_u16(metrics, glyph.min(count - 1) * 4).unwrap();
    let offset = if glyph < count {
      glyph * 4 + 2
    } else {
      count * 4 + (glyph - count) * 2
    };
    (
      advance,
      i16::from_be_bytes([metrics[offset], metrics[offset + 1]]),
    )
  }
  #[test]
  fn ttf_and_cff_subsets_preserve_cjk_vertical_metrics_and_ligature_closure() {
    for original in [
      include_bytes!("../test/fixtures/SyntheticCJK.ttf").as_slice(),
      include_bytes!("../test/fixtures/SyntheticCJK.otf").as_slice(),
    ] {
      let chars: HashSet<char> = "中文fiAV".chars().collect();
      let prepared = PreparedFont::new(original.to_vec()).unwrap();
      let encoded = prepared.subset(&chars).unwrap();
      let decoded = normalize_font(&encoded, 0).unwrap();
      let source_face =
        hb_subset::FontFace::new(hb_subset::Blob::from_bytes(original).unwrap()).unwrap();
      let subset_face =
        hb_subset::FontFace::new(hb_subset::Blob::from_bytes(&decoded).unwrap()).unwrap();
      let source_map = source_face.nominal_glyph_mapping().unwrap();
      let subset_map = subset_face.nominal_glyph_mapping().unwrap();
      assert_eq!(subset_face.covered_codepoints().unwrap().len(), chars.len());
      // Six requested characters + .notdef + the GSUB fi ligature.
      assert_eq!(subset_face.glyph_count(), chars.len() + 2);
      for ch in chars {
        let source = source_map.get(ch).unwrap() as usize;
        let subset = subset_map.get(ch).unwrap() as usize;
        assert_eq!(
          metric(original, source, false),
          metric(&decoded, subset, false)
        );
        assert_eq!(
          metric(original, source, true),
          metric(&decoded, subset, true)
        );
      }
    }
  }

  fn make_woff(sfnt: &[u8]) -> Vec<u8> {
    let num = read_u16(sfnt, 4).unwrap() as usize;
    let mut out = vec![0u8; 44 + num * 20];
    out[0..4].copy_from_slice(b"wOFF");
    out[4..8].copy_from_slice(&read_u32(sfnt, 0).unwrap().to_be_bytes());
    out[12..14].copy_from_slice(&(num as u16).to_be_bytes());
    for i in 0..num {
      let rec = 12 + i * 16;
      let tag = &sfnt[rec..rec + 4];
      let offset = read_u32(sfnt, rec + 8).unwrap() as usize;
      let len = read_u32(sfnt, rec + 12).unwrap() as usize;
      let table = &sfnt[offset..offset + len];
      let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
      encoder.write_all(table).unwrap();
      let zipped = encoder.finish().unwrap();
      let zipped_len = zipped.len();
      let (payload, compressed_len) = if zipped_len < len {
        (zipped, zipped_len)
      } else {
        (table.to_vec(), len)
      };
      let at = 44 + i * 20;
      out[at..at + 4].copy_from_slice(tag);
      let payload_at = out.len() as u32;
      out[at + 4..at + 8].copy_from_slice(&payload_at.to_be_bytes());
      out[at + 8..at + 12].copy_from_slice(&(compressed_len as u32).to_be_bytes());
      out[at + 12..at + 16].copy_from_slice(&(len as u32).to_be_bytes());
      out[at + 16..at + 20].copy_from_slice(&read_u32(sfnt, rec + 4).unwrap().to_be_bytes());
      out.extend_from_slice(&payload);
    }
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_be_bytes());
    out
  }

  #[test]
  fn woff1_normalizer_preserves_sfnt_tables() {
    let Ok(path) = std::env::var("CJK_TEST_FONT") else {
      return;
    };
    let source = fs::read(path).unwrap();
    let normalized = normalize_woff(&make_woff(&source)).unwrap();
    assert_eq!(&normalized[..4], &source[..4]);
    assert!(subset_to_woff2(&normalized, &HashSet::from(['A'])).is_ok());
  }

  #[test]
  fn ttc_face_selection_checks_index() {
    let Ok(path) = std::env::var("CJK_TEST_TTC") else {
      return;
    };
    let ttc = fs::read(path).unwrap();
    assert!(normalize_ttc(&ttc, 1).is_ok());
    assert!(normalize_ttc(&ttc, 999).is_err());
  }
}
