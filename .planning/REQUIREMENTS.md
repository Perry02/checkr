# Requirements: Smarter Compiler Test Case Generators

**Defined:** 2026-03-22
**Core Value:** Every GCL construct a student's compiler must handle is exercised — no correct implementation goes unvalidated, no bug passes undetected by accident.

## v1 Requirements

### Generator Design

- [ ] **GEN-01**: `gcl_compiler_gen.rs` is refactored into a unified API with an internal weighted catalog of scenarios
- [ ] **GEN-02**: Entry point `gen_commands` picks a scenario (random, skip-focused, loop-focused, nested, array, non-deterministic) then generates a program guaranteeing that scenario's construct appears
- [ ] **GEN-03**: `Generate for Input` in `ce-compiler/src/lib.rs` caller signature unchanged

### Targeted Generators

- [ ] **TGT-01**: `gen_skip_program` — guaranteed `skip` command
- [ ] **TGT-02**: `gen_simple_if` — single-guard `if b -> cmd fi`
- [ ] **TGT-03**: `gen_multi_guard_if(n)` — n-guard `if` (n=2,3)
- [ ] **TGT-04**: `gen_simple_do` — single-guard `do b -> cmd od` with loop-done edge
- [ ] **TGT-05**: `gen_do_n_guards(n)` — n-guard `do` (n=2,3,4) targeting `donegc` computation
- [ ] **TGT-06**: `gen_nested_if_in_do` — `if` inside `do`
- [ ] **TGT-07**: `gen_nested_do_in_if` — `do` inside `if`
- [ ] **TGT-08**: `gen_array_assignment` — `A[expr] := expr`
- [ ] **TGT-09**: `gen_array_read` — `x := A[expr]`
- [ ] **TGT-10**: `gen_non_deterministic_overlapping` — overlapping guards in non-deterministic mode
- [ ] **TGT-11**: `gen_variable_reuse` — same variable assigned in multiple guards
- [ ] **TGT-12**: `gen_variable_as_index` — variable used as both array index and scalar in same program

### Oracle Improvements

- [ ] **ORC-01**: Co-generation — alongside each program, produce witness memories that guarantee each guard evaluates both true and false
- [ ] **ORC-02**: Replace or augment action bag validation with path-based fingerprinting — trace all paths from initial node, fingerprint edge label sequences, preventing structural collisions

### Proof / Evaluation

- [ ] **PRF-01**: Level 1 — written code inspection argument: `gcl_gen.rs` cannot produce `skip`, arrays, or unary minus (cite exact lines)
- [ ] **PRF-02**: Level 2 — Rust test generating N programs with each generator, counting construct frequencies, printing comparison table
- [ ] **PRF-03**: Level 3 — synthetic buggy `Compiler.fs` variant that mishandles `skip`; show it passes old generator, fails new one

## v2 Requirements

### Future Environments

- **ENV-01**: Extend catalog-based approach to sign analysis environment
- **ENV-02**: Extend to security analysis environment

### Stretch

- **STR-01**: Shrinking — minimize a failing test to the smallest program that still fails

## Out of Scope

| Feature | Reason |
|---------|--------|
| Parse error testing | Tests the student's parser, not program graph construction — different compiler stage |
| fm4fun comparison | Baseline is `gcl_gen.rs`, not fm4fun |
| Other environments (v1) | Compiler only for now |
| Modifying real student submissions | Use synthetic buggy impl for mutation testing |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| GEN-01, GEN-02, GEN-03 | Phase 1 | Pending |
| TGT-01 – TGT-12 | Phase 1 | Pending |
| ORC-01 | Phase 2 | Pending |
| ORC-02 | Phase 2 | Pending |
| PRF-01 – PRF-03 | Phase 3 | Pending |

**Coverage:**
- v1 requirements: 18 total
- Mapped to phases: 18
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after initial definition*
