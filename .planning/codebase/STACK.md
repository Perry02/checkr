# Technology Stack

**Analysis Date:** 2026-03-22

## Languages

**Primary:**
- Rust (Edition 2024) - All backend crates, CLI tools, WASM library
- TypeScript - Frontend apps (`apps/inspectify/`, `apps/chip/`)
- Svelte 5 - UI component language for both frontend apps

**Secondary:**
- F# - External student starter kit (`starters/fsharp-starter/` git submodule)

## Runtime

**Environment:**
- Rust backend: native binary (tokio async runtime)
- WASM target: `wasm32-unknown-unknown` compiled via wasm-pack
- Frontend: Node.js v20.11.0 (pinned via `.nvmrc`)

**Package Manager:**
- Rust: Cargo (workspace resolver v2), lockfile `Cargo.lock` present
- Node: npm, lockfile `package-lock.json` present (lockfileVersion 3)

## Frameworks

**Backend (Rust):**
- `axum` 0.8.1 (with `macros`, `ws` features) - HTTP server and WebSocket support in `crates/inspectify/`
- `tokio` 1.37.0 (full features) - Async runtime across all backend crates
- `tapi` (git: `https://github.com/oeb25/tapi.git`, features: `chrono`, `endpoints`, `smol_str`, `toml`) - Type-safe API layer that auto-generates TypeScript client and F# types
- `tower-http` 0.6.2 (cors) - CORS middleware for the API server
- `clap` 4.4.4 (derive) - CLI argument parsing in `crates/checkr/`, `crates/chip-cli/`, `crates/inspectify/`, `crates/xtask/`

**Parser/Compiler tooling:**
- `lalrpop` 0.22.1 - Parser generator (build-dep in `crates/gcl/` and `crates/chip/`)
- `lalrpop-util` 0.22.1 (lexer) - Runtime for LALRPOP-generated parsers

**Frontend:**
- SvelteKit 2.17.1 (`apps/inspectify/`), SvelteKit 2.20.2 (`apps/chip/`)
- Svelte 5.19.7 (`apps/inspectify/`), Svelte ^5.0.0 (`apps/chip/`)
- Vite 6.1.0 (`apps/inspectify/`), Vite 6.2.2 (`apps/chip/`)
- TailwindCSS 4.x - Utility CSS for both frontends
- Monaco Editor - Code editor widget in both frontend apps
- `vis-network` 9.1.9 - Graph visualization in both frontend apps
- `katex` 0.16.9 - LaTeX math rendering in `apps/chip/`
- `immer` 10.0.3 - Immutable state updates in `apps/inspectify/`

**Testing:**
- `cargo nextest` - Rust test runner (installed via `taiki-e/install-action@nextest` in CI)
- `insta` 1.38.0 - Snapshot testing in `crates/mcltl-rs/`
- `vitest` ^1.3.1 - Frontend test runner in `apps/inspectify/`

**Build/Dev:**
- `just` - Task runner (`Justfile` at project root); manages wasm-pack builds, dev servers, releases
- `wasm-pack` - Compiles `crates/chip-wasm/` to WebAssembly package
- `wasm-bindgen` 0.2.87 - Rust/WASM JS interop in `crates/chip-wasm/`
- `cargo dist` 0.28.1 - Cross-platform binary release tooling (config in `dist-workspace.toml`)
- `cargo release` - Version bumping and tag creation (config in `Cargo.toml` `[workspace.metadata.release]`)
- `git-cliff` - CHANGELOG generation (config in `cliff.toml`)
- `cargo zigbuild` - Cross-compilation for patching binaries to remote targets (used in `Justfile`)
- `rustfmt` - Code formatter (config in `rustfmt.toml`)
- `clippy` - Linter (run in CI at MSRV 1.85.1)

## Key Dependencies

**Critical:**
- `smtlib` 0.3.0 - SMT solver interface used by `crates/chip/` and `crates/chip-cli/`; chip-cli enables `tokio` feature
- `rusqlite` 0.33.0 (bundled, chrono) - Embedded SQLite database in `crates/inspectify/` for the `checko` grading mode
- `petgraph` 0.7.1 - Graph data structure used by `crates/gcl/` and `crates/envs/ce-compiler/`
- `graphviz-rust` 0.9.3 - Graphviz DOT output in `crates/envs/ce-compiler/`
- `z3-solver` 4.13.0 (pinned) - Z3 SMT solver JS bindings, used by `apps/chip/` via WASM SharedArrayBuffer
- `chip-wasm` - Local WASM package (`file:../../crates/chip-wasm/pkg`) consumed by `apps/chip/`

**Infrastructure:**
- `serde` 1.0.152 (derive, rc) + `serde_json` 1.0.91 - Serialization across all crates
- `lz4_flex` 0.11.2 - Compression for cached run data in `crates/inspectify/`
- `rust-embed` 8.3.0 (axum, compression) - Embeds built frontend into the `inspectify` binary at compile time
- `notify` 8.0.0 + `notify-debouncer-mini` 0.6.0 - File system watching in `crates/driver/`
- `tracing` 0.1.37 + `tracing-subscriber` 0.3.16 + `tracing-error` 0.2.0 - Structured logging stack
- `color-eyre` 0.6.2 - Error reporting with backtraces
- `reqwest` 0.12.15 (json) - HTTP client in `crates/xtask/` for tooling scripts
- `camino` 1.1.6 - UTF-8 typed paths
- `chrono` 0.4.33 (serde) - Date/time handling in `crates/gitty/` and `crates/inspectify/`
- `dashmap` 6.1.0 - Concurrent hash map in `crates/ce-shell/`
- `rand` 0.9.0 (small_rng) - Random generation for test input synthesis
- `miette` 7.5.0 (fancy, serde) - Diagnostic error formatting in `crates/gcl/`, `crates/chip/`, `crates/chip-cli/`, `crates/chip-wasm/`
- `mcltl` (internal crate at `crates/mcltl-rs/`) - Model checking LTL; depends on `smol_str`, `ahash`, `smallvec`

## Configuration

**Environment:**
- No `.env` files detected in the repository
- Frontend env vars injected at build time via Vite (`PUBLIC_API_BASE`, `PUBLIC_CHECKO`, `PUBLIC_INSPECTIFY_VERSION`)
- `PUBLIC_API_BASE` controls where the frontend calls the backend API
- `PUBLIC_CHECKO` enables checko (grading) mode in the frontend
- `RUST_LOG` controls backend log verbosity (e.g., `RUST_LOG=debug`)
- `CHECKO_REMOTE_HOST`, `CHECKO_REMOTE_PATH`, `WIN_REMOTE_HOST`, `WIN_REMOTE_PATH` used by deployment `just` targets

**Build:**
- `Cargo.toml` - Workspace manifest with shared dependency versions
- `.cargo/config.toml` - Sets `net.git-fetch-with-cli = true`; adds `xtask` alias; configures `wasm32-unknown-unknown` rustflags
- `rustfmt.toml` - Formatter config (unstable features, grouped imports, field init shorthand)
- `dist-workspace.toml` - cargo-dist release config (targets, installers, updater)
- `Justfile` - All common development tasks (dotenv-load enabled)

## Platform Requirements

**Development:**
- Rust stable (MSRV: 1.85.1)
- Node.js 20.11.0
- `wasm-pack` for building `apps/chip/`
- `just` for task automation

**Production:**
- `inspectify` binary: self-contained; embeds the `apps/inspectify/build/` frontend at compile time
- Release targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`
- Installers: shell script and PowerShell generated by cargo-dist
- `chip` app: static site (SvelteKit adapter-static), deployed to GitHub Pages

---

*Stack analysis: 2026-03-22*
