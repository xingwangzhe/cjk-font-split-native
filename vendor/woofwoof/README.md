# Local woofwoof fork

Based on bearcove/woofwoof 1.0.2, retaining its Google WOFF2 transforms, wrapper and dual license. The Brotli shim forwards to Google Brotli 1.2 through compu-brotli-sys 1.2.0 rather than Rust brotli 7. Compression quality, window and FONT mode are preserved. Cargo's patch applies the same codec to fontcull's decoder to avoid duplicate WOFF2 symbols.
