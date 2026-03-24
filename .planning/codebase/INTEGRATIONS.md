# External Integrations

**Analysis Date:** 2026-03-22

## APIs & External Services

**SMT Solvers:**
- Z3 (via `smtlib` 0.3.0) - Used by `crates/chip/` and `crates/chip-cli/` for satisfiability checking in Hoare logic verification
  - SDK/Client: `smtlib` crate (workspace dep, `tokio` feature enabled in `chip-cli`)
  - Auth: None (local process execution)
- Z3 (JavaScript/WASM, via `z3-solver` 4.13.0 pinned) - Used by `apps/chip/` in the browser
  - SDK/Client: `z3-solver` npm package
  - Auth: None
  - Note: Requires `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers for SharedArrayBuffer; configured in `apps/chip/static/netlify.toml`

**Graphviz:**
- `graphviz-rust` 0.9.3 - DOT graph generation in `crates/envs/ce-compiler/`; output is rendered in the frontend via `vis-network`

## Data Storage

**Databases:**
- SQLite (embedded, bundled) - Used in `crates/inspectify/` for the `checko` grading/competition mode
  - Connection: file path supplied via `--checko <path>` CLI argument to `inspectify` binary
  - Client: `rusqlite` 0.33.0 with `bundled` feature (SQLite compiled into the binary, no system install required)
  - Schema: single table `cached_runs (cache_key TEXT PRIMARY KEY, data BLOB NOT NULL)`; data column stores LZ4-compressed, JSON-serialized `JobData`
  - Implementation: `crates/inspectify/src/checko/db.rs`

**File Storage:**
- Local filesystem only - run outputs, group configs, and student submission results are stored as files on disk
- `crates/driver/` uses `notify` for file system watching to detect changes to `run.toml` and source files

**Caching:**
- In-process SQLite cache (`crates/inspectify/src/checko/db.rs`) - caches the results of running student submissions to avoid re-running on restart

## Authentication & Identity

**Auth Provider:**
- None detected - no authentication layer on the HTTP API
- The `inspectify` server binds to `127.0.0.1` only by default (`crates/inspectify/src/main.rs` line 138), limiting exposure to localhost
- CORS is set to permissive (`tower_http::cors::CorsLayer::permissive()`) for the `/api` router

## Monitoring & Observability

**Error Tracking:**
- None (no external service such as Sentry)

**Logs:**
- `tracing` + `tracing-subscriber` stack across all Rust crates
- Log level controlled by `RUST_LOG` env var (default: `INFO`)
- `hyper` target is filtered out explicitly in `inspectify` (`crates/inspectify/src/main.rs`)
- `tracing-wasm` 0.2.1 used in `crates/chip-wasm/` to route tracing to the browser console

## CI/CD & Deployment

**Hosting:**
- `inspectify` binary: distributed as GitHub Releases via `cargo dist` 0.28.1
  - Targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`
  - Installers: shell (`install.sh`), PowerShell (`install.ps1`), with a self-updater
  - Binary repository (for patching workflow): `git@github.com:team-checkr/inspectify-binaries.git`
- `chip` app: static site deployed to GitHub Pages via `peaceiris/actions-gh-pages@v4`
  - Target repository: `team-checkr/team-checkr.github.io` (external repo, `main` branch)
  - Build output: `apps/chip/build/`
- Checko server: deployed via `scp` + `ssh` to a remote Linux host; env vars `CHECKO_REMOTE_HOST` and `CHECKO_REMOTE_PATH` configure the target

**CI Pipeline:**
- GitHub Actions (`.github/workflows/ci.yml`) - Runs on push to `main` and all PRs
  - Jobs: `test` (Linux/Windows/macOS matrix), `check` (MSRV 1.85.1 + debug), `lockfile`, `docs`, `rustfmt`, `clippy`
  - Test runner: `cargo nextest` (`taiki-e/install-action@nextest`)
  - Cache: `Swatinem/rust-cache@v2`
- GitHub Actions (`.github/workflows/release.yml`) - Triggered on version tags (`**[0-9]+.[0-9]+.[0-9]+*`)
  - Uses `cargo dist` 0.28.1 to build and publish GitHub Releases
  - Builds the `apps/inspectify/` frontend with `npm run build` before packaging the Rust binary
  - Secrets used: `GITHUB_TOKEN` (built-in)
- GitHub Actions (`.github/workflows/pages.yml`) - Deploys `apps/chip/` to GitHub Pages on push to `main`
  - Secrets used: `PERSONAL_TOKEN` (for pushing to external GitHub Pages repo)
  - Installs `wasm-bindgen`, `wasm-pack`, `just` via `taiki-e/install-action@v2`

## Environment Configuration

**Required env vars at build time (frontend):**
- `PUBLIC_API_BASE` - Base URL for backend API calls (empty string in release = same origin)
- `PUBLIC_CHECKO` - Set to non-empty string to enable checko grading mode in UI

**Optional runtime env vars (backend):**
- `RUST_LOG` - Log filter directive (e.g., `debug`, `info`)

**Deployment env vars (developer machine, loaded via `Justfile` dotenv):**
- `CHECKO_REMOTE_HOST` - SSH host for checko server deployment
- `CHECKO_REMOTE_PATH` - Remote path for checko binary
- `WIN_REMOTE_HOST` - SSH host for Windows machine deployment
- `WIN_REMOTE_PATH` - Remote path for Windows binary

**Secrets location:**
- `GITHUB_TOKEN` - GitHub Actions built-in secret
- `PERSONAL_TOKEN` - GitHub Actions repository secret (used for Pages deployment)
- No `.env` files committed to the repository

## Webhooks & Callbacks

**Incoming:**
- None detected - `inspectify` is a local development tool and grading server; no inbound webhooks

**Outgoing:**
- None detected - all external interactions are initiated by the user (running student programs, reading files)

## External Repositories & Submodules

**Git Submodule:**
- `starters/fsharp-starter` → `git@github.com:team-checkr/fsharp-starter.git`
  - F# starter kit for student submissions; types are auto-generated from the Rust API via `tapi` and written to `starters/fsharp-starter/src/Io.fs` by `inspectify` at startup

**Binary Mirror Repository:**
- `git@github.com:team-checkr/inspectify-binaries.git` - Stores pre-built binaries for distribution outside of GitHub Releases; updated by `patch-inspectify-binaries-macos` just target and the `update-inspectify-binaries` target pulls from GitHub Releases

**tapi Library:**
- Source: `https://github.com/oeb25/tapi.git` (git dependency, no pinned ref)
- Purpose: Generates TypeScript API client (`apps/inspectify/src/lib/api.ts`) and F# type definitions automatically from Rust endpoint types

---

*Integration audit: 2026-03-22*
