# Smarter Compiler Test Case Generators for Inspectify

## What This Is

A redesign of the GCL compiler test case generator in Inspectify, replacing the old random sampler (`gcl_gen.rs`) with a smarter, catalog-based generator (`gcl_compiler_gen.rs`) that guarantees coverage of all GCL compiler constructs. The work also includes a three-level empirical argument proving the old generator is insufficient and the new one is measurably better. Scope is limited to the compiler environment for now.

## Core Value

Every GCL construct that a student's compiler must handle is exercised by at least one generated test case — no correct implementation goes unvalidated, no bug goes undetected by accident.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Refactor `gcl_compiler_gen.rs` into a unified API with an internal weighted catalog of scenarios
- [ ] Implement targeted generators that guarantee each GCL construct appears: `skip`, simple `if`, multi-guard `if`, simple `do...od`, nested `if` inside `do`, nested `do` inside `if`, array assignment, array read, non-deterministic overlapping guards
- [ ] Level 1 proof: code inspection argument showing `gcl_gen.rs` structurally cannot produce `skip`, arrays, or unary minus
- [ ] Level 2 proof: statistical test that generates N programs with each generator and counts construct frequencies, producing a comparison table
- [ ] Level 3 proof: mutation test showing a deliberately buggy student implementation passes all old-generator tests but is caught by the new generator
- [ ] The unified entry point (`Generate for Input` in `ce-compiler`) calls the catalog-based generator with no API change to the caller

### Out of Scope

- Other environments (sign analysis, security, etc.) — compiler only for now, other envs planned later
- fm4fun comparison — not relevant, baseline is `gcl_gen.rs`
- Shrinking / minimization of failing tests — stretch goal, deferred
- Modifying actual student submissions for mutation testing — use a controlled synthetic buggy implementation instead

## Context

- `gcl_gen.rs` is the original shared generator, used across environments. It provably cannot produce `skip` (no arm in the `Command` generator), array targets (`use_array()` hardcoded to `false`), or unary minus. It is still the conceptual baseline for the thesis argument.
- `gcl_compiler_gen.rs` is the student's work-in-progress sketch — already integrated into `ce-compiler/src/lib.rs` as the active generator. It adds `skip`, arrays, and unary minus over `gcl_gen.rs`, but is still a pure random sampler with no construct guarantees. Needs refactoring into the catalog design.
- The compiler environment takes GCL commands + determinism setting → produces a program graph in DOT format. Validation fingerprints each edge using 10 random memory samples and compares against the reference graph.
- A tester branch (implemented by a group member) compares generators on some metric — exact metric TBD, but the three proof levels should map onto it.
- More student implementations will arrive — the generator needs to work well across diverse `Compiler.fs` implementations, not just the one example already present.

## Constraints

- **Tech stack**: Rust, existing `rand` crate, `gcl` AST types — no new dependencies
- **API**: `Generate for Input` caller signature must not change — the catalog is an internal detail
- **Scope**: Compiler environment only for now

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Unified API, catalog internally | Simple for callers, guarantees construct coverage probabilistically, tunable weights, stronger thesis argument than pure random or pure targeted | — Pending |
| Targeted generators live in `gcl_compiler_gen.rs` | Keeps compiler-specific generation in one place, the current file is already the right home | — Pending |
| Synthetic buggy impl for Level 3 proof | Avoids modifying real student submissions, keeps the mutation test controlled and reproducible | — Pending |
| Separate targeted suite for thesis evaluation (Option B) | Cleaner to argue in thesis — N targeted tests each guarantee a specific construct | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-22 after initialization*
