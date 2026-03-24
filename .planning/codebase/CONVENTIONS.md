# Code Conventions

**Analysis Date:** 2026-03-22

## Language Standards

- **Rust Edition:** 2024 (`edition = "2024"` in workspace `Cargo.toml`)
- **MSRV:** 1.85.1 (enforced in CI `check` job)
- **TypeScript:** strict mode via SvelteKit defaults
- **Formatter:** `rustfmt` with unstable features enabled (`rustfmt.toml`); grouped imports, field init shorthand
- **Linter:** `clippy` run in CI at MSRV

## Rust Conventions

### Crate organization

- Each analysis environment = one crate in `crates/envs/ce-<name>/`
- `src/lib.rs` is the only source file for env crates — `Input`, `Output`, and `impl Env` all in one file
- Shared utilities go in `crates/stdx/`; domain types go in `crates/gcl/`

### Trait-based design

```rust
// Define analysis via Env trait
impl Env for InterpreterEnv {
    type Input = Input;
    type Output = Output;
    type Meta = BTreeSet<TargetDef>;
    fn run(input: &Self::Input) -> Result<Self::Output> { ... }
    fn validate(input: &Self::Input, output: &Self::Output) -> Result<ValidationResult> { ... }
    fn meta(input: &Self::Input) -> Self::Meta { ... }
}
```

### Macro-based registration

```rust
// Register a new env with one line in ce-shell/src/lib.rs
define_shell!(
    ce_interpreter::InterpreterEnv[Interpreter, "Interpreter"],
    // ...
);
```

### Derive macros

All API-crossing types derive `tapi::Tapi`, `serde::Serialize`, `serde::Deserialize`, `Clone`, `Debug`, `PartialEq`.

```rust
#[derive(tapi::Tapi, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[tapi(path = "Interpreter")]
pub struct Input { ... }
```

### Error handling

- `color-eyre` with `eyre::Result` and `wrap_err` / `wrap_err_with` for context at call sites
- `thiserror` for domain error enums (e.g., `EnvError`)
- `miette` for user-facing diagnostics with source spans (gcl, chip parsers)
- Functions returning `Result<T>` use the workspace-defined `color_eyre::Result` alias in binaries; `ce_core::Result<T, EnvError>` in library code

### Async

- `tokio` runtime (full features) used throughout backend
- `#[tracing::instrument]` on most public async functions with `skip_all` + relevant fields

### Naming

- Types: `PascalCase`
- Functions/methods: `snake_case`
- Constants: `UPPER_SNAKE_CASE`
- Crates: `kebab-case` directories, `snake_case` in Rust (underscores)
- Generic metadata parameter: conventionally `M` (e.g., `Hub<M>`, `Driver<M>`, `Job<M>`)

## Frontend (Svelte/TypeScript) Conventions

### Component style

- Svelte 5 runes syntax (`$state`, `$derived`, `$effect`)
- TailwindCSS 4.x utility classes inline in templates
- Components in `src/lib/` are shared; route components in `src/routes/`

### API client

- `apps/inspectify/src/lib/api.ts` is **auto-generated** by `tapi` — never edit manually
- Regenerated when `inspectify` server starts (writes F# types too)

### State management

- `immer` for immutable state updates in `apps/inspectify/`
- Local `$state` for component state; no global store for most cases

## Tracing / Logging

```rust
#[tracing::instrument(skip_all, fields(analysis = self.to_string()))]
pub fn validate_output(&self, output: &Output) -> Result<ValidationResult, EnvError> { ... }
```

Standard pattern: `skip_all` to avoid logging large data, then explicit `fields(...)` for searchable keys.

## `run.toml` Config Pattern

Student implementations are configured via `run.toml`:

```toml
[run]
cmd = "./my-binary"

[compile]
cmd = "cargo build --release"
```

`Driver` reads this file and uses `notify` to watch for changes.

---

*Conventions analysis: 2026-03-22*
