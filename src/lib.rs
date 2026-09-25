#![deny(clippy::all)]

use flate2::read::ZlibDecoder;
use napi::bindgen_prelude::Buffer;
use napi::{Error, Result, Status};
use napi_derive::napi;
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const ALGORITHM_VERSION: &str = "fontcull-2;allsorts-cff-1;woff-normalizer-1;woff2-q8";
const WOFF2_QUALITY: usize = 8;
static CACHE_LOCK: Mutex<()> = Mutex::new(());

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
  let chars: HashSet<char> = text.chars().collect();
  if chars.is_empty() {
    return Err(Error::new(
      Status::InvalidArg,
      "text must contain at least one character",
    ));
  }
  let face = face_index.unwrap_or(0);
  let mut sorted: Vec<char> = chars.iter().copied().collect();
  sorted.sort_unstable();

  let mut hasher = blake3::Hasher::new();
  hasher.update(ALGORITHM_VERSION.as_bytes());
  hasher.update(&face.to_le_bytes());
  hasher.update(&(font.len() as u64).to_le_bytes());
  hasher.update(&font);
  for ch in &sorted {
    hasher.update(&(*ch as u32).to_le_bytes());
  }
  let key = hasher.finalize().to_hex().to_string();
  let cache = PathBuf::from(cache_dir);
  fs::create_dir_all(&cache).map_err(io_error)?;
  let output = cache.join(format!("{key}.woff2"));

  let _guard = CACHE_LOCK
    .lock()
    .map_err(|_| Error::new(Status::GenericFailure, "cache lock poisoned"))?;
  if output.is_file() {
    let bytes = fs::metadata(&output).map_err(io_error)?.len() as u32;
    return Ok(SubsetResult {
      path: output.to_string_lossy().into_owned(),
      hash: key,
      cache_hit: true,
      bytes,
      characters: sorted.len() as u32,
    });
  }

  let normalized = normalize_font(&font, face).map_err(|e| Error::new(Status::InvalidArg, e))?;
  let woff2 =
    subset_to_woff2(&normalized, &chars).map_err(|e| Error::new(Status::GenericFailure, e))?;
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

fn subset_to_woff2(font: &[u8], chars: &HashSet<char>) -> std::result::Result<Vec<u8>, String> {
  use allsorts::binary::read::ReadScope;
  use allsorts::font::MatchingPresentation;
  use allsorts::font_data::FontData;
  use allsorts::subset::{self, CmapTarget, SubsetProfile};
  use allsorts::tables::FontTableProvider;
  use allsorts::unicode::VariationSelector;

  let parsed = ReadScope::new(font)
    .read::<FontData>()
    .map_err(|e| format!("font parse failed: {e:?}"))?;
  let provider = parsed
    .table_provider(0)
    .map_err(|e| format!("font tables unavailable: {e:?}"))?;
  if provider.has_table(allsorts::tag::CFF) || provider.has_table(allsorts::tag::CFF2) {
    let mut mapped_font =
      allsorts::Font::new(provider).map_err(|e| format!("CFF cmap parse failed: {e:?}"))?;
    let mut glyphs = Vec::with_capacity(chars.len() + 1);
    glyphs.push(0);
    for ch in chars {
      let (glyph, _) = mapped_font.lookup_glyph_index(
        *ch,
        MatchingPresentation::NotRequired,
        Option::<VariationSelector>::None,
      );
      glyphs.push(glyph);
    }
    glyphs.sort_unstable();
    glyphs.dedup();
    let subset = subset::subset(
      &mapped_font.font_table_provider,
      &glyphs,
      &SubsetProfile::Minimal,
      CmapTarget::Unicode,
    )
    .map_err(|e| format!("CFF subset failed: {e:?}"))?;
    woofwoof::compress(&subset, "", WOFF2_QUALITY, true)
      .ok_or_else(|| "CFF WOFF2 compression failed".to_string())
  } else {
    let subset = fontcull::subset_font_data(font, chars, &[]).map_err(|e| e.to_string())?;
    woofwoof::compress(&subset, "", WOFF2_QUALITY, true)
      .ok_or_else(|| "WOFF2 compression failed".to_string())
  }
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
    Some(b"wOF2") => fontcull::decompress_font(data)
      .map_err(|e| e.to_string())
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
    assert!(fontcull::subset_font_to_woff2(&normalized, &HashSet::from(['A']), &[]).is_ok());
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
