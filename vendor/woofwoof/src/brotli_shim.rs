//! Forward the existing WOFF2 C ABI to the statically bundled Google Brotli.
use std::ffi::c_int;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn woofwoof_BrotliEncoderCompress(
  quality: c_int,
  lgwin: c_int,
  mode: c_int,
  input_size: usize,
  input_buffer: *const u8,
  encoded_size: *mut usize,
  encoded_buffer: *mut u8,
) -> c_int {
  if input_buffer.is_null() || encoded_size.is_null() || encoded_buffer.is_null() {
    return 0;
  }
  // WOFF2 owns these buffers and passes their allocated lengths unchanged.
  unsafe {
    compu_brotli_sys::BrotliEncoderCompress(
      quality,
      lgwin,
      mode,
      input_size,
      input_buffer,
      encoded_size,
      encoded_buffer,
    )
  }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn woofwoof_BrotliDecoderDecompress(
  encoded_size: usize,
  encoded_buffer: *const u8,
  decoded_size: *mut usize,
  decoded_buffer: *mut u8,
) -> c_int {
  if encoded_buffer.is_null() || decoded_size.is_null() || decoded_buffer.is_null() {
    return 0;
  }
  unsafe {
    compu_brotli_sys::BrotliDecoderDecompress(
      encoded_size,
      encoded_buffer,
      decoded_size,
      decoded_buffer,
    )
  }
}
