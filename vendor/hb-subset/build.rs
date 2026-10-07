fn main() {
  let mut build = cc::Build::new();
  build.opt_level(3);
  if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
    build.flag_if_supported("-funroll-loops");
  }
  if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
    build.flag("/bigobj");
  }
  build
    .cpp(true)
    .std("c++17")
    .warnings(false)
    .file("harfbuzz/src/harfbuzz-subset.cc")
    .compile("embedded-harfbuzz-subset");
  println!("cargo:rerun-if-changed=harfbuzz/src");
}
