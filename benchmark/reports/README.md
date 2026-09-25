# Benchmark reports

The 2026-09-24 Node 24.21.0 Linux x64 run used the 25,645,604-byte LXGW WenKai TTF and 665 codepoints. Each cold, warm, and `subset-font` measurement is the median of three runs. The Vite figure is a three-page static build median.

| Pipeline | WOFF2 quality | Cold median | Warm/cache median | Output | Vite build median |
| --- | ---: | ---: | ---: | ---: | ---: |
| Rust, initial comparison | 11 | 320.45 ms | 7.74 ms | 151,308 bytes | 56.71 ms |
| Rust, selected | 8 | 42.75 ms | 6.98 ms | 159,892 bytes | 84.81 ms |
| `subset-font` WASM | package default | 491.75 ms | — | 123,408 bytes | — |

The q8 cold subset was about 11.5× faster than the WASM baseline in this run, with output about 29.5% larger. Timings vary by machine; raw measurements and environment are in the adjacent JSON files. The selected cache check hit on all three repeated page requests.
