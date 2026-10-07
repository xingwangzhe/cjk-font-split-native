# Runtime-focused native builds

Release builds use `opt-level = 3`, full (`fat`) LTO, one code-generation unit, and disabled incremental compilation. The CPU instruction baseline and panic-unwind behavior remain compatible with the existing package.

`bun run build:pgo` additionally builds an instrumented addon, executes deterministic representative workloads, merges its raw counters with the toolchain-matched `llvm-profdata`, then builds the final addon with `-Cprofile-use`. Install the tools with `rustup component add llvm-tools-preview` first. Training runs on a matching native host; Linux x64 musl training uses Node 24 in an Alpine container. For the font package GNU builds, use `--zig-glibc 2.28` to preserve the matching C/C++ sysroot and glibc baseline.

PGO applies to the Rust dependency graph. It does not apply to the font package's separately compiled C/C++ libraries. The release retains full LTO across Rust crates; no cross-language LTO or fast-math assumptions are introduced.

Use `bun run benchmark:pgo` (Node 24+, Bun, Rust, LLVM tools) to build an unprofiled baseline and the PGO candidate and compare them. Workload inputs differ between training and measurement. Reports use four Rayon threads, an ABBAAB process order, and the median of 21 warm samples per workload, with each sample calibrated to roughly 30 ms. They describe these synthetic workloads on the current machine, not a universal speedup. Profiles and binary sizes are not committed.

Build records and runtime reports are saved under `target/pgo/<target>/`. CI uploads the build record for each platform and tests the final binary before publishing. To train locally for an explicit target, add `--target <triple>` on a matching host. Do not use `target-cpu=native` for distributed npm binaries: that changes CPU compatibility to the build machine.

The `scripts/pgo-workload.mjs` corpus should be updated alongside APIs or representative workloads; PGO is retrained from source for every release, using the same compiler for profile generation and use. Normal `bun run build` remains an unprofiled release build for development and comparison.

CI uses `bun run build:pgo --select`: it also builds the unprofiled release and measures both variants on each target. After two discarded warmup processes, it selects PGO only when the balanced workload geometric mean is at least 3% faster and no non-trivial workload (baseline at least 20 microseconds) regresses by more than 15%. Otherwise it publishes the unprofiled binary. This conservative sample-based selection is not a proof about all workloads; the per-target `build.json` records the selected variant and the comparison is uploaded as `benchmark.json`. The instrumented binary is never selected for publication.
