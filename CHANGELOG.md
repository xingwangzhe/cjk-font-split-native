# Changelog

## 0.4.12 — 2026-11-18

- Split native bindings into platform-specific scoped npm packages; the root package now ships the matching platform dependency only.
- Publish platform packages before the root package through the release workflow.

## 0.4.11 — 2026-11-18

- Upgrade napi-rs, Vite, font processing, and compatible transitive dependencies.
- Verify Rust, TypeScript, native binding, plugin, and benchmark checks locally.
