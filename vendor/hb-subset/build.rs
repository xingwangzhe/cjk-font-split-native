fn main() {
  cc::Build::new()
    .cpp(true)
    .std("c++17")
    .warnings(false)
    .file("harfbuzz/src/harfbuzz-subset.cc")
    .compile("embedded-harfbuzz-subset");
  println!("cargo:rerun-if-changed=harfbuzz/src");
}
