# Roadmap: Smarter Compiler Test Case Generators

## Overview

Three phases taking the GCL compiler test generation from a pure random sampler to a catalog-based generator with oracle improvements and a three-level empirical proof for the thesis.

## Phases

- [ ] **Phase 1: Catalog Generator** - Refactor gcl_compiler_gen.rs into a unified catalog-based generator with all targeted generators implemented
- [ ] **Phase 2: Oracle Improvements** - Fix the two identified oracle weaknesses: fixed-seed memory sampling and action bag collision
- [ ] **Phase 3: Three-Level Proof** - Produce all evaluation artifacts for the thesis argument

## Phase Details

### Phase 1: Catalog Generator
**Goal**: Refactor `gcl_compiler_gen.rs` into a unified catalog-based generator where the entry point picks a scenario and delegates to a targeted generator that guarantees that construct appears
**Depends on**: Nothing (first phase)
**Requirements**: GEN-01, GEN-02, GEN-03, TGT-01, TGT-02, TGT-03, TGT-04, TGT-05, TGT-06, TGT-07, TGT-08, TGT-09, TGT-10, TGT-11, TGT-12
**Success Criteria** (what must be TRUE):
  1. `gen_commands` entry point internally picks a scenario from a weighted catalog
  2. Every targeted generator (TGT-01 through TGT-12) is implemented and reachable from the catalog
  3. `Generate for Input` in `ce-compiler/src/lib.rs` caller signature is unchanged
  4. Running the generator 100 times produces programs containing skip, arrays, loops, and nested constructs
**Plans:** 3 plans

Plans:
- [ ] 01-01-PLAN.md — Scenario enum, weighted CATALOG, gen_commands dispatcher with stubs
- [ ] 01-02-PLAN.md — Targeted generators TGT-01 through TGT-07 (skip, if variants, do variants, nesting)
- [ ] 01-03-PLAN.md — Targeted generators TGT-08 through TGT-12 (arrays, non-determinism, variable reuse)

### Phase 2: Oracle Improvements
**Goal**: Fix the two oracle weaknesses — replace fixed-seed 10-memory sampling with co-generated witness memories, and replace action bag comparison with path-based fingerprinting
**Depends on**: Phase 1
**Requirements**: ORC-01, ORC-02
**Success Criteria** (what must be TRUE):
  1. `Input` carries witness memories alongside the GCL program
  2. Validation uses witness memories instead of the fixed-seed 10-sample oracle
  3. Path-based fingerprinting is implemented and replaces or augments action bag comparison
  4. A program with overlapping guards in deterministic mode cannot produce the same fingerprint as the non-deterministic variant
**Plans:** 2 plans

Plans:
- [ ] 02-01-PLAN.md — Co-generated witness memories (Input struct, collect_guards, generate_witness_memories, validate fallback)
- [ ] 02-02-PLAN.md — Path-based fingerprinting (DFS path traversal, augmented validate with dual comparison)

### Phase 3: Three-Level Proof
**Goal**: Produce all evaluation artifacts proving the new generator is measurably better than gcl_gen.rs
**Depends on**: Phase 2
**Requirements**: PRF-01, PRF-02, PRF-03
**Success Criteria** (what must be TRUE):
  1. Level 1 argument is documented with exact line citations from gcl_gen.rs
  2. Level 2 Rust test runs and prints a construct frequency comparison table (old vs new)
  3. Level 3 synthetic buggy implementation exists, passes old generator, fails new generator
**Plans**: TBD

Plans:
- [ ] 03-01: Level 1 (code inspection) and Level 2 (statistical test)
- [ ] 03-02: Level 3 (mutation test with synthetic buggy implementation)

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Catalog Generator | 0/3 | Planned | - |
| 2. Oracle Improvements | 0/2 | Planned | - |
| 3. Three-Level Proof | 0/2 | Not started | - |
