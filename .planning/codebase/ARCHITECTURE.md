# Architecture

**Analysis Date:** 2026-03-22

## Pattern

**Plugin-based layered architecture** with 8 distinct layers:

1. **Language/IR** (`crates/gcl/`) — Guarded Command Language parser, AST, program graph (PG) generation, interpreter
2. **Environment Interface** (`crates/ce-core/`) — `Env` trait defining the contract for analysis environments: `Input`, `Output`, `Meta`, `validate()`, `run()`
3. **Analysis Environments** (`crates/envs/ce-*/`) — Individual analysis implementations (Calculator, Parser, Compiler, Interpreter, Security, Sign Analysis); each is a separate crate implementing `Env`
4. **Shell/Aggregation** (`crates/ce-shell/`) — Type-erases all environments via the `define_shell!` macro; creates unified `Analysis` enum, type-erased `Input`/`Output`/`Meta`, and dispatch logic
5. **Driver** (`crates/driver/`) — Manages subprocess lifecycle: spawns student processes from `run.toml`, watches filesystem for changes, tracks `Job<M>` state via `Hub<M>`
6. **Reference Binary** (`crates/checkr/`) — CLI tool that runs a single analysis and outputs JSON; used as the "reference implementation" against which student outputs are validated
7. **Server/API** (`crates/inspectify/`) — axum HTTP server with `tapi`-generated type-safe API; manages `Driver`, exposes endpoints, embeds frontend at compile time; `checko` sub-module handles batch grading
8. **Frontend** (`apps/inspectify/`) — SvelteKit SPA consuming the `tapi`-generated TypeScript client; Monaco editor + vis-network graph rendering

**Secondary binary:** `apps/chip/` — standalone SvelteKit app for Hoare logic verification using `crates/chip-wasm/` (Rust compiled to WASM) + Z3 WASM solver.

## Layers & Data Flow

### Single-developer inspection flow

```
User (browser)
  → SvelteKit frontend (apps/inspectify/)
    → HTTP/WebSocket (tapi-generated client)
      → inspectify server (crates/inspectify/)
        → Driver<InspectifyJobMeta>
          → Hub<M> (in-memory event bus)
          → spawn student subprocess (run.toml config)
            → student binary writes JSON to stdout
          → ce-shell parses output bytes → Output
          → Input.validate_output() → ValidationResult
        → SSE stream of HubEvents back to frontend
```

### Checko batch grading flow

```
checko config (groups.toml + programs.toml)
  → Checko struct (crates/inspectify/src/checko.rs)
    → one Driver per student group
    → SQLite cache (db.rs) — avoids re-running on restart
    → scoreboard computed from all group results
    → /checko/public endpoint → frontend scoreboard view
```

## Key Abstractions

### `Env` trait (`crates/ce-core/src/lib.rs`)

```rust
pub trait Env {
    type Input: ...;
    type Output: ...;
    type Meta: ...;
    fn run(input: &Self::Input) -> Result<Self::Output>;
    fn validate(input: &Self::Input, output: &Self::Output) -> Result<ValidationResult>;
    fn meta(input: &Self::Input) -> Self::Meta;
}
```

The core contract. Every analysis implements this.

### `define_shell!` macro (`crates/ce-shell/src/def.rs`)

```rust
define_shell!(
    ce_calculator::CalcEnv[Calculator, "Calculator"],
    ce_parser::ParserEnv[Parser, "Parser"],
    // ...
);
```

Generates: `Analysis` enum, type-erased `Input`/`Output`/`Meta` wrappers, and all dispatch match arms. **Adding a new analysis = one line here + new `crates/envs/ce-*` crate.**

### `Hub<M>` + `Driver<M>` (`crates/driver/src/`)

- `Hub<M>` — shared in-memory broadcast channel; holds all `Job<M>` states; `M` is caller-defined metadata (e.g., `InspectifyJobMeta`)
- `Driver<M>` — reads `run.toml` for compile/run commands; spawns subprocesses; updates Hub on job state changes; watches filesystem for `run.toml` changes

### Type-erased `Input`/`Output` (`crates/ce-shell/src/io.rs`)

Wrapped with a content `Hash` for cheap equality/cache keying. Memoized validation via `DashMap<(Hash, Hash), ValidationResult>`.

### `tapi` auto-generated API (`crates/inspectify/src/endpoints.rs`)

`#[tapi::tapi(path = "/generate", method = Post)]` on Rust handlers auto-generates TypeScript client at `apps/inspectify/src/lib/api.ts` and F# types at `starters/fsharp-starter/src/Io.fs`.

## Entry Points

| Binary | Entry | Purpose |
|--------|-------|---------|
| `inspectify` | `crates/inspectify/src/main.rs` | Main dev server + grading server |
| `checkr` | `crates/checkr/src/main.rs` | CLI reference runner |
| `chip-cli` | `crates/chip-cli/src/main.rs` | CLI Hoare logic verifier |
| `xtask` | `crates/xtask/src/main.rs` | Cargo build automation tasks |

## Deployment Modes

| Mode | How to activate | Behavior |
|------|-----------------|----------|
| **Inspectify** (dev) | `inspectify --run run.toml` | Watches one student repo, real-time UI |
| **Checko** (grading) | `inspectify --checko checko.db --groups groups.toml` | Batch-runs all groups, scoreboard |
| **chip** (browser) | Static site on GitHub Pages | WASM-only, no backend |

---

*Architecture analysis: 2026-03-22*
