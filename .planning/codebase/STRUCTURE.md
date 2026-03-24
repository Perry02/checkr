# Directory Structure

**Analysis Date:** 2026-03-22

## Top-Level Layout

```
checkr/
├── apps/
│   ├── chip/           # Hoare logic verifier — SvelteKit static site
│   └── inspectify/     # Main dev/grading UI — SvelteKit + axum backend
├── crates/
│   ├── ce-core/        # Env trait + core types (EnvError, ValidationResult, Generate)
│   ├── ce-shell/       # define_shell! macro — aggregates all analysis envs
│   ├── checkr/         # CLI binary — runs reference analysis, outputs JSON
│   ├── chip/           # Hoare logic verifier library (Rust)
│   ├── chip-cli/       # CLI Hoare logic verifier
│   ├── chip-wasm/      # chip compiled to WASM for apps/chip/
│   ├── driver/         # Subprocess manager: Hub<M>, Driver<M>, Job<M>
│   ├── envs/           # Individual analysis environments (see below)
│   ├── example/        # Example programs.toml + groups.toml for testing
│   ├── gcl/            # Guarded Command Language: parser, AST, PG, interpreter
│   ├── gitty/          # Git history utilities (used by checko)
│   ├── inspectify/     # axum server, checko grading, tapi endpoints
│   ├── mcltl-rs/       # LTL model checker
│   ├── stdx/           # Internal stdlib extensions (Stringify wrapper, etc.)
│   └── xtask/          # Cargo xtask automation (CI helpers, new-env scaffolding)
├── starters/
│   └── fsharp-starter/ # Git submodule: F# student starter kit
├── student_implementation/  # Student submission examples (Group-03)
├── .cargo/config.toml  # Workspace Cargo config (net, aliases, wasm target)
├── .github/workflows/  # CI/CD: ci.yml, release.yml, pages.yml
├── Cargo.toml          # Workspace manifest + shared dependency versions
├── Justfile            # Task runner (build, dev, release, deploy targets)
├── dist-workspace.toml # cargo-dist release config
├── run.toml            # Default run config for single-env development
└── rustfmt.toml        # Formatter config
```

## Analysis Environments (`crates/envs/`)

```
crates/envs/
├── ce-calculator/  # Expression calculator analysis
├── ce-compiler/    # GCL→bytecode compiler + graphviz DOT output
├── ce-interpreter/ # GCL interpreter with step-by-step trace
├── ce-parser/      # GCL parser — parse tree output
├── ce-security/    # Security lattice analysis
└── ce-sign/        # Sign analysis (abstract interpretation)
```

Each env crate has the same structure:
```
crates/envs/ce-<name>/
├── Cargo.toml
└── src/
    └── lib.rs      # defines Input, Output, impl Env for <Name>Env
```

## Frontend Layout (`apps/inspectify/`)

```
apps/inspectify/
├── src/
│   ├── lib/
│   │   ├── api.ts          # Auto-generated tapi TypeScript client (do not edit)
│   │   └── *.svelte        # Shared components
│   └── routes/
│       └── (inspectify)/
│           ├── +layout.svelte
│           └── env/[analysis]/
│               └── +page.svelte   # Per-analysis route page
├── package.json
└── vite.config.ts
```

## Key File Locations

| File | Purpose |
|------|---------|
| `crates/ce-shell/src/lib.rs` | **`define_shell!` invocation** — add new analysis here |
| `crates/ce-shell/src/def.rs` | `define_shell!` macro definition |
| `crates/ce-core/src/lib.rs` | `Env` trait definition |
| `crates/inspectify/src/main.rs` | Server entry point, port binding, axum router setup |
| `crates/inspectify/src/endpoints.rs` | All tapi API endpoints + `AppState` |
| `crates/inspectify/src/checko.rs` | Checko grading orchestration |
| `crates/inspectify/src/checko/db.rs` | SQLite cache for run results |
| `crates/driver/src/lib.rs` | `Driver<M>` and `Hub<M>` types |
| `apps/inspectify/src/lib/api.ts` | **Auto-generated** — do not edit manually |
| `run.toml` | Default student run config (compile + run commands) |
| `crates/example/groups.toml` | Example checko groups config |
| `crates/example/programs.toml` | Example checko programs config |

## How to Add a New Analysis Environment

1. Create `crates/envs/ce-<name>/` crate implementing `Env`
2. Add one line to `define_shell!` in `crates/ce-shell/src/lib.rs`
3. Add route page at `apps/inspectify/src/routes/(inspectify)/env/<Name>/+page.svelte`
4. Add workspace member to `Cargo.toml` `[workspace]`

## Naming Conventions

- Rust crates: `kebab-case` (dirs), `snake_case` (Rust module names)
- Analysis env crates: `ce-<name>` prefix
- Env structs: `<Name>Env` (e.g., `InterpreterEnv`)
- Frontend components: `PascalCase.svelte`
- API routes: snake_case paths (e.g., `/exec_analysis`)

---

*Structure analysis: 2026-03-22*
