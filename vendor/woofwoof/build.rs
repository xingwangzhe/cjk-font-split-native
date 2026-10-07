fn main() {
  // Compile the woff2 C++ library + our C wrapper
  let mut build = cc::Build::new();
  build.opt_level(3);
  if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
    build.flag_if_supported("-funroll-loops");
  }
  build.cpp(true).std("c++11");
  if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
    build.flag("/FIcstdint");
  } else {
    build.flags(&["-include", "cstdint"]);
  }
  build
    .include("vendor/woff2/include")
    .include("include") // Our brotli stub headers
    // woff2 source files (excluding CLI tools and fuzzers)
    .file("vendor/woff2/src/font.cc")
    .file("vendor/woff2/src/glyph.cc")
    .file("vendor/woff2/src/normalize.cc")
    .file("vendor/woff2/src/table_tags.cc")
    .file("vendor/woff2/src/transform.cc")
    .file("vendor/woff2/src/variable_length.cc")
    .file("vendor/woff2/src/woff2_common.cc")
    .file("vendor/woff2/src/woff2_dec.cc")
    .file("vendor/woff2/src/woff2_enc.cc")
    .file("vendor/woff2/src/woff2_out.cc")
    // Our C-compatible wrapper
    .file("wrapper/woff2_wrapper.cpp")
    .warnings(false)
    .compile("woff2");
}
