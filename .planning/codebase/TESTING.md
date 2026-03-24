# Testing

**Analysis Date:** 2026-03-22

## Test Runners

- **Rust:** `cargo nextest` (installed via `taiki-e/install-action@nextest` in CI)
- **Frontend:** `vitest` ^1.3.1 in `apps/inspectify/`

## Test Structure

### Rust — inline module tests

Tests live in `#[cfg(test)]` modules within source files or sibling `tests.rs` files:

```
crates/envs/ce-interpreter/src/
├── lib.rs          # main impl
└── tests.rs        # #[cfg(test)] module, imported via `mod tests;`
```

```rust
// tests.rs pattern
use ce_core::{Env, ValidationResult};
use crate::{Input, InterpreterEnv, Output};

#[test]
fn initially_stuck_program() {
    let input = Input { commands: Stringify::Unparsed("if false -> skip fi".to_string()), ... };
    let output = InterpreterEnv::run(&input).unwrap();
    match InterpreterEnv::validate(&input, &output).unwrap() {
        ValidationResult::Correct => (),
        _ => panic!(),
    }
}
```

### Snapshot tests (`insta`)

Used exclusively in `crates/mcltl-rs/`:

```rust
// insta snapshot pattern
insta::assert_debug_snapshot!(result);
```

Snapshots stored in `crates/mcltl-rs/src/snapshots/`. Update with `cargo insta review`.

### Rust test locations

| Crate | Test type | Location |
|-------|-----------|----------|
| `ce-interpreter` | Unit + integration | `src/tests.rs` |
| `gcl` | Unit | inline `#[cfg(test)]` blocks |
| `mcltl-rs` | Unit + snapshot | inline + `src/*/` with `insta` |
| `gitty` | Unit | inline `#[cfg(test)]` |

### No tests in

- `ce-core`, `ce-shell` — pure trait/macro definitions
- `checkr`, `inspectify`, `driver` — integration requires running processes
- `ce-calculator`, `ce-compiler`, `ce-parser`, `ce-security`, `ce-sign` — no test files found

## How Analysis Tests Work

Tests call `Env::run()` directly and then `Env::validate()` to check correctness — no subprocess involved:

```rust
let output = InterpreterEnv::run(&input).unwrap();
match InterpreterEnv::validate(&input, &output).unwrap() {
    ValidationResult::Correct => (),
    ValidationResult::Mismatch { reason } => panic!("reason: {reason:?}"),
    ValidationResult::TimeOut => panic!(),
}
```

This tests the reference implementation independently of the student subprocess.

## CI Test Matrix

From `.github/workflows/ci.yml`:

- **Platforms:** Linux (`ubuntu-latest`), Windows, macOS
- **Jobs:**
  - `test` — `cargo nextest run` (all platforms)
  - `check` — `cargo check` at MSRV 1.85.1
  - `lockfile` — `cargo update --locked` check
  - `rustfmt` — `cargo fmt --check`
  - `clippy` — `cargo clippy`
  - `docs` — `cargo doc --no-deps`

## Frontend Tests

- Framework: `vitest` in `apps/inspectify/`
- No test files found yet — testing setup present but tests not written
- `apps/chip/` — no test framework configured

## Mocking

- No mock frameworks used
- Tests call real `Env::run()` and `Env::validate()` implementations
- No HTTP mocking — integration tests for the server are not present

---

*Testing analysis: 2026-03-22*
