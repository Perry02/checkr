# Phase 1: Catalog Generator - Research

**Researched:** 2026-03-22
**Domain:** Rust random generation, GCL AST construction, weighted catalog dispatch
**Confidence:** HIGH

---

## Summary

Phase 1 refactors `gcl_compiler_gen.rs` from a pure random sampler into a catalog-based generator. The entry point `gen_commands` picks a scenario from a weighted list and delegates to a targeted generator function that GUARANTEES the named construct appears. All 12 targeted generators (TGT-01 through TGT-12) are implemented in the same file and registered in the catalog.

The codebase is already well-prepared for this refactor. The `ErasedRng`/bridge pattern, the `CompilerContext` struct, the `gen_*` free functions, and the weighted `sample` dispatch mechanism are all in place. The only structural change needed is: (1) introduce a `Scenario` enum, (2) add a weighted catalog that maps scenarios to targeted generator functions, (3) implement each targeted generator so it constructs the guaranteed AST node directly rather than relying on probability.

The `Generate for Input` signature in `ce-compiler/src/lib.rs` (line 103-113) calls `gen_commands(&mut CompilerContext::default(), rng)` and must not change. All work is internal to `gcl_compiler_gen.rs`.

**Primary recommendation:** Keep `ErasedRng` and `CompilerContext` exactly as they are. Add a `Scenario` enum, a `CATALOG` array of `(f32, Scenario)` pairs, and implement each targeted generator as a free `fn` that takes `&mut CompilerContext, &mut impl Rng` and returns `Commands`.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| GEN-01 | `gcl_compiler_gen.rs` refactored into unified API with weighted catalog | Catalog dispatch pattern documented in §Architecture Patterns |
| GEN-02 | `gen_commands` picks a scenario then delegates to a targeted generator guaranteeing that construct | Entry-point refactor documented with concrete signatures |
| GEN-03 | `Generate for Input` in `ce-compiler/src/lib.rs` caller signature unchanged | Caller code analyzed — only touches `gen_commands` and `CompilerContext::default()` |
| TGT-01 | `gen_skip_program` — guaranteed `skip` command | AST: `Command::Skip`; construction documented |
| TGT-02 | `gen_simple_if` — single-guard `if b -> cmd fi` | AST: `Command::If(vec![Guard(...)])` |
| TGT-03 | `gen_multi_guard_if(n)` — n-guard `if` (n=2,3) | AST: `Command::If(vec![Guard; n])` |
| TGT-04 | `gen_simple_do` — single-guard `do b -> cmd od` with loop-done edge | AST: `Command::Loop(vec![Guard(...)])` |
| TGT-05 | `gen_do_n_guards(n)` — n-guard `do` (n=2,3,4) targeting `donegc` computation | AST: `Command::Loop(vec![Guard; n])`; determinism interaction documented |
| TGT-06 | `gen_nested_if_in_do` — `if` inside `do` | Nesting pattern: guard body contains `Command::If` |
| TGT-07 | `gen_nested_do_in_if` — `do` inside `if` | Nesting pattern: guard body contains `Command::Loop` |
| TGT-08 | `gen_array_assignment` — `A[expr] := expr` | `Target::Array(Array("A"), Box::new(expr))` as LHS |
| TGT-09 | `gen_array_read` — `x := A[expr]` | `AExpr::Reference(Target::Array(...))` as RHS |
| TGT-10 | `gen_non_deterministic_overlapping` — overlapping guards in non-det mode | Two guards whose boolean exprs share variable; `Determinism` is chosen at caller, generator produces AST |
| TGT-11 | `gen_variable_reuse` — same variable assigned in multiple guards | Same `Variable("x")` in LHS of multiple guard bodies |
| TGT-12 | `gen_variable_as_index` — variable used as both array index and scalar | `A[x]` as array target, `x := ...` as scalar assignment in same program |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rand` | already in Cargo.toml | RNG, weighted selection, `SeedableRng` | Already used; `IndexedRandom::choose_weighted` is the weighted dispatch mechanism |
| `gcl::ast::*` | workspace | GCL AST types | The types we construct |

### No New Dependencies
No new crates are needed. Everything required already exists in the workspace.

---

## Architecture Patterns

### Current Module Layout
```
crates/ce-core/src/
├── gn.rs                    # trait Generate, module re-exports
├── gn/
│   ├── gcl_gen.rs           # old shared generator (trait impls) — DO NOT TOUCH
│   └── gcl_compiler_gen.rs  # compiler-specific generator — ALL WORK HERE
crates/envs/ce-compiler/src/
└── lib.rs                   # caller — DO NOT TOUCH (signature constraint)
```

### Pattern 1: Weighted Scenario Catalog
**What:** A static or const array of `(weight: f32, Scenario)` pairs. `gen_commands` samples from this array then dispatches to the matching targeted generator.
**When to use:** Entry point only — callers see no change, catalog is purely internal.

```rust
// Source: synthesized from existing sample() pattern in gcl_compiler_gen.rs
#[derive(Clone, Copy)]
enum Scenario {
    Skip,
    SimpleIf,
    MultiGuardIf2,
    MultiGuardIf3,
    SimpleDo,
    DoNGuards2,
    DoNGuards3,
    DoNGuards4,
    NestedIfInDo,
    NestedDoInIf,
    ArrayAssignment,
    ArrayRead,
    NonDeterministicOverlapping,
    VariableReuse,
    VariableAsIndex,
    Random,              // fallback: existing behavior
}

const CATALOG: &[(f32, Scenario)] = &[
    (1.0, Scenario::Skip),
    (2.0, Scenario::SimpleIf),
    (2.0, Scenario::MultiGuardIf2),
    (1.0, Scenario::MultiGuardIf3),
    (2.0, Scenario::SimpleDo),
    (1.5, Scenario::DoNGuards2),
    (1.0, Scenario::DoNGuards3),
    (0.5, Scenario::DoNGuards4),
    (1.5, Scenario::NestedIfInDo),
    (1.5, Scenario::NestedDoInIf),
    (2.0, Scenario::ArrayAssignment),
    (2.0, Scenario::ArrayRead),
    (1.0, Scenario::NonDeterministicOverlapping),
    (1.0, Scenario::VariableReuse),
    (1.0, Scenario::VariableAsIndex),
    (3.0, Scenario::Random),
];
```

### Pattern 2: Targeted Generator Signature
**What:** Each targeted generator is a free function. It constructs the required construct directly (not via probability), then optionally wraps it in random prefix/suffix commands.
**When to use:** Every TGT-* requirement.

```rust
// Targeted generators take the context and rng — same signature as existing gen_*
pub fn gen_skip_program<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    // Guarantee: Commands ALWAYS contains Command::Skip
    let mut cmds = vec![Command::Skip];
    // Optionally prepend/append random assignments for variety
    if rng.random_bool(0.5) {
        cmds.insert(0, Command::Assignment(gen_target(cx, rng), gen_aexpr(cx, rng)));
    }
    Commands(cmds)
}
```

### Pattern 3: Refactored gen_commands Entry Point
**What:** `gen_commands` becomes a thin dispatcher — sample scenario, call targeted generator. Signature unchanged.

```rust
pub fn gen_commands<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let scenario = CATALOG
        .choose_weighted(rng, |item| item.0)
        .unwrap()
        .1;
    match scenario {
        Scenario::Skip                      => gen_skip_program(cx, rng),
        Scenario::SimpleIf                  => gen_simple_if(cx, rng),
        Scenario::MultiGuardIf2             => gen_multi_guard_if(cx, rng, 2),
        Scenario::MultiGuardIf3             => gen_multi_guard_if(cx, rng, 3),
        Scenario::SimpleDo                  => gen_simple_do(cx, rng),
        Scenario::DoNGuards2                => gen_do_n_guards(cx, rng, 2),
        Scenario::DoNGuards3                => gen_do_n_guards(cx, rng, 3),
        Scenario::DoNGuards4                => gen_do_n_guards(cx, rng, 4),
        Scenario::NestedIfInDo              => gen_nested_if_in_do(cx, rng),
        Scenario::NestedDoInIf              => gen_nested_do_in_if(cx, rng),
        Scenario::ArrayAssignment           => gen_array_assignment(cx, rng),
        Scenario::ArrayRead                 => gen_array_read(cx, rng),
        Scenario::NonDeterministicOverlapping => gen_non_deterministic_overlapping(cx, rng),
        Scenario::VariableReuse             => gen_variable_reuse(cx, rng),
        Scenario::VariableAsIndex           => gen_variable_as_index(cx, rng),
        Scenario::Random                    => Commands(cx.many(1, 10, rng, gen_command)),
    }
}
```

Note: `CATALOG.choose_weighted` works because `CATALOG` is a slice — `use rand::seq::IndexedRandom` is already imported.

### Pattern 4: ErasedRng — Keep As-Is
**What:** The existing `ErasedRng = SmallRng` alias and `bridge()` function exist to make boxed closures work around `rand::Rng`'s dyn-incompatibility. The `sample()` method on `CompilerContext` is already used by the existing generators.
**Decision:** Keep unchanged. Targeted generators do NOT need `sample()` or boxed closures because they construct AST nodes directly. They use the caller's `R: Rng` directly.
**Why:** Targeted generators build structure intentionally — they don't need weighted random dispatch internally (or only need it for minor sub-expressions, which can call existing `gen_aexpr`, `gen_bexpr`, etc.).

### Pattern 5: CompilerContext — Force vs. Disable Flags
**What:** Current flags are all "no_*" (disablement). Targeted generators need to FORCE constructs on.
**Solution:** Do NOT add "force_*" flags to `CompilerContext`. Instead, targeted generators bypass the context flags by constructing the required AST node directly and calling sub-generators only for filler content. The "no_*" flags remain as guards for the random sub-generators.

```rust
pub fn gen_array_assignment<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    // FORCE: construct array target directly, ignoring cx.no_arrays
    let arr_name = cx.array_names
        .first()
        .cloned()
        .unwrap_or_else(|| "A".into());
    let idx = gen_aexpr(cx, rng);
    let rhs = gen_aexpr(cx, rng);
    let guaranteed = Command::Assignment(
        Target::Array(Array(arr_name), Box::new(idx)),
        rhs,
    );
    Commands(vec![guaranteed])
}
```

### Anti-Patterns to Avoid
- **Relying on probability to produce the guaranteed construct:** A targeted generator that passes `no_arrays: false` and hopes `gen_target` picks an array is NOT a guarantee. The construct must be hard-coded in the targeted generator.
- **Adding `force_*` flags to CompilerContext:** Unnecessary complexity. Direct construction is simpler and more correct.
- **Removing `ErasedRng`/`bridge`:** The existing `gen_aexpr`, `gen_bexpr`, `gen_command` etc. still use it; removing it would require rewriting all existing generators.
- **Using `cx.many` with `gen_command_erased` for targeted commands:** `cx.many` is probabilistic. Targeted generators should build the `Vec<Command>` manually and then wrap in `Commands(...)`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Weighted random choice | Custom weighted roulette | `slice.choose_weighted(rng, |item| item.0)` from `rand::seq::IndexedRandom` | Already imported; handles edge cases |
| Random arithmetic expressions | Re-implement expression generation | Call existing `gen_aexpr(cx, rng)` for filler | Existing generator already handles recursion limits, unary minus, arrays |
| Random boolean expressions | Re-implement bool generation | Call existing `gen_bexpr(cx, rng)` for filler | Already handles negation limits |
| Random scalar target | Re-implement reference generation | Call existing `gen_target(cx, rng)` or `gen_reference` for non-guaranteed targets | Already handles array vs variable selection |

---

## Code Examples

### GCL AST — Key Constructors
```rust
// Source: crates/gcl/src/ast.rs and crates/gcl/src/ast_ext.rs

// Command variants
Command::Skip
Command::Assignment(Target<Box<AExpr>>, AExpr)
Command::If(Vec<Guard>)
Command::Loop(Vec<Guard>)

// Guard: boolean condition + body commands
Guard(BExpr, Commands)

// Target variants
Target::Variable(Variable("x".into()))
Target::Array(Array("A".into()), Box::new(idx_expr))

// AExpr constructors
AExpr::Number(42)
AExpr::Reference(target)          // variable or array read
AExpr::binary(lhs, AOp::Plus, rhs) // convenience from ast_ext
AExpr::Minus(Box::new(expr))      // unary minus

// BExpr constructors
BExpr::Bool(true)
BExpr::Rel(lhs_aexpr, RelOp::Gt, rhs_aexpr)
BExpr::logic(lhs, LogicOp::And, rhs)  // convenience from ast_ext
BExpr::Not(Box::new(bexpr))
```

### gen_simple_if skeleton
```rust
// TGT-02: guaranteed single-guard if
pub fn gen_simple_if<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let body = Commands(vec![gen_command(cx, rng)]);
    let guard = Guard(gen_bexpr(cx, rng), body);
    Commands(vec![Command::If(vec![guard])])
}
```

### gen_simple_do skeleton
```rust
// TGT-04: guaranteed single-guard do..od
// The program graph for Command::Loop emits a loop-done edge automatically (see pg.rs:211-214)
pub fn gen_simple_do<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let body = Commands(vec![gen_command(cx, rng)]);
    let guard = Guard(gen_bexpr(cx, rng), body);
    Commands(vec![Command::Loop(vec![guard])])
}
```

### gen_nested_if_in_do skeleton
```rust
// TGT-06: if inside do
pub fn gen_nested_if_in_do<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let inner_guard = Guard(gen_bexpr(cx, rng), Commands(vec![gen_command(cx, rng)]));
    let inner_if = Command::If(vec![inner_guard]);
    let outer_guard = Guard(gen_bexpr(cx, rng), Commands(vec![inner_if]));
    Commands(vec![Command::Loop(vec![outer_guard])])
}
```

### gen_array_read skeleton
```rust
// TGT-09: x := A[expr]
pub fn gen_array_read<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let arr_name = cx.array_names.first().cloned().unwrap_or_else(|| "A".into());
    let idx = gen_aexpr(cx, rng);
    let rhs = AExpr::Reference(Target::Array(Array(arr_name), Box::new(idx)));
    let scalar_var = cx.names.first().cloned().unwrap_or_else(|| "x".into());
    let lhs = Target::Variable(Variable(scalar_var));
    Commands(vec![Command::Assignment(lhs, rhs)])
}
```

### gen_non_deterministic_overlapping skeleton
```rust
// TGT-10: overlapping guards — generator produces two guards where both conditions
// reference the same variable with overlapping ranges. Determinism is chosen at
// the CALLER level (ce-compiler/src/lib.rs line 104-106) and is NOT controlled here.
// The generator just guarantees two structurally overlapping boolean conditions.
pub fn gen_non_deterministic_overlapping<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let var = cx.names.first().cloned().unwrap_or_else(|| "a".into());
    let v = AExpr::Reference(Target::Variable(Variable(var)));
    // Both guards can be true when the variable is in overlapping range
    let b1 = BExpr::Rel(v.clone(), RelOp::Gt, AExpr::Number(0));
    let b2 = BExpr::Rel(v, RelOp::Lt, AExpr::Number(10));
    let g1 = Guard(b1, Commands(vec![gen_command(cx, rng)]));
    let g2 = Guard(b2, Commands(vec![gen_command(cx, rng)]));
    Commands(vec![Command::If(vec![g1, g2])])
}
```

### gen_variable_reuse skeleton
```rust
// TGT-11: same variable assigned in multiple guards
pub fn gen_variable_reuse<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let var = cx.names.first().cloned().unwrap_or_else(|| "a".into());
    let lhs = || Target::Variable(Variable(var.clone()));
    let g1 = Guard(
        gen_bexpr(cx, rng),
        Commands(vec![Command::Assignment(lhs(), gen_aexpr(cx, rng))]),
    );
    let g2 = Guard(
        gen_bexpr(cx, rng),
        Commands(vec![Command::Assignment(lhs(), gen_aexpr(cx, rng))]),
    );
    Commands(vec![Command::If(vec![g1, g2])])
}
```

### gen_variable_as_index skeleton
```rust
// TGT-12: variable used as both array index and scalar
pub fn gen_variable_as_index<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    let var = cx.names.first().cloned().unwrap_or_else(|| "a".into());
    let arr_name = cx.array_names.first().cloned().unwrap_or_else(|| "A".into());
    // Command 1: A[a] := expr  (variable 'a' as array index)
    let idx = AExpr::Reference(Target::Variable(Variable(var.clone())));
    let arr_assign = Command::Assignment(
        Target::Array(Array(arr_name), Box::new(idx)),
        gen_aexpr(cx, rng),
    );
    // Command 2: a := expr  (same variable 'a' as scalar)
    let scalar_assign = Command::Assignment(
        Target::Variable(Variable(var)),
        gen_aexpr(cx, rng),
    );
    Commands(vec![arr_assign, scalar_assign])
}
```

---

## How Program Graph Edges Map to AST (for TGT-04/TGT-05 context)

From `crates/gcl/src/pg.rs` (lines 203-226), the edge semantics are:

```
Command::Skip          → one edge: (s, Action::Skip, t)
Command::Assignment    → one edge: (s, Action::Assignment(...), t)
Command::If(guards)    → guard_edges(...) only (no loop-back)
Command::Loop(guards)  → guard_edges(...) PLUS one extra edge (s, Action::Condition(done_expr), t)
```

The "loop-done" edge is the `done_expr` in `Command::Loop` — it fires when ALL guard conditions are false. For `gen_do_n_guards(n)`, having n guards means the `done` expression is the conjunction of n negated conditions: `!b1 && !b2 && ... && !bn`. This is what exercises the `donegc` computation in student compilers.

In Deterministic mode, each guard condition for `if`/`do` is additionally qualified by "none of the previous guards were true" (see `guard_edges` at pg.rs:163-186). This means multi-guard programs generate more complex condition edges and exercise a wider part of the compiler.

---

## Common Pitfalls

### Pitfall 1: Targeted Generator Calls gen_command Which Might Not Produce the Target
**What goes wrong:** A "targeted" generator that adds the construct probabilistically still fails to guarantee it if `gen_command` picks a different branch.
**Why it happens:** `gen_command` uses weighted random dispatch — it can generate any `Command` variant.
**How to avoid:** Construct the guaranteed AST node inline in the targeted generator. Only use `gen_command` for FILLER commands (e.g., a random assignment before the guaranteed construct).
**Warning signs:** If the targeted generator contains `gen_command` at a position where the guaranteed construct is expected, it is wrong.

### Pitfall 2: Empty array_names When Generating Array Constructs
**What goes wrong:** `cx.array_names.first()` returns `None`, causing `unwrap_or_else` fallback or panic.
**Why it happens:** `CompilerContext::default()` sets `array_names` to `["A", "B", "C"]` but callers could construct a custom context.
**How to avoid:** Always use `unwrap_or_else(|| "A".into())` on `first()`. Array generators are safe with the default context.

### Pitfall 3: choose_weighted Import
**What goes wrong:** `CATALOG.choose_weighted(rng, ...)` fails to compile.
**Why it happens:** `choose_weighted` is on the `IndexedRandom` trait, which must be in scope.
**How to avoid:** The existing import `use rand::seq::IndexedRandom;` at the top of `gcl_compiler_gen.rs` is sufficient. Do not remove it.

### Pitfall 4: Scenario Enum Copy/Clone Requirements
**What goes wrong:** `match scenario { ... }` fails because `Scenario` doesn't implement `Copy`.
**Why it happens:** The `choose_weighted` call returns a reference to the tuple; you need to extract the scenario value.
**How to avoid:** Derive `#[derive(Clone, Copy)]` on the `Scenario` enum. All variants are unit variants so this is free.

### Pitfall 5: gen_do_n_guards with Fuel Exhaustion
**What goes wrong:** Generating n guards inside a `do` loop with deep nested expressions exhausts `cx.fuel`, causing subsequent sub-generators to produce trivial programs.
**Why it happens:** `cx.fuel` is decremented by `many()` and recursive generators.
**How to avoid:** For targeted generators, set `cx.fuel` high enough before generating guards, or construct guards directly without going through `cx.many`. Since targeted generators control construction, prefer direct construction: `vec![Guard(...); n]` built with explicit loops.

### Pitfall 6: Determinism is Chosen at the Caller, Not the Generator
**What goes wrong:** A generator for TGT-10 (non-deterministic overlapping) tries to set determinism mode internally — but `Determinism` is part of `Input`, not `Commands`.
**Why it happens:** The program graph construction (in `ce-compiler/src/lib.rs`) takes `input.determinism` separately from `input.commands`. The generator only produces `Commands`.
**How to avoid:** TGT-10's generator produces AST with overlapping boolean conditions. Whether those become overlapping edges depends on the `Determinism` chosen at the caller. The generator's job is to produce the overlapping AST structure — the caller's random determinism selection handles the rest. Document this clearly in the function's doc comment.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[test]`) |
| Config file | none — standard `cargo test` |
| Quick run command | `cargo test -p ce-compiler` |
| Full suite command | `cargo test` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| GEN-01/02 | `gen_commands` runs 100 times; every scenario is reached | Rust `#[test]` with frequency counting | `cargo test -p ce-compiler construct_coverage` | No — Wave 0 |
| GEN-03 | `Generate for Input` compiles and produces valid `Input` | Compile-time check (existing code must still compile) | `cargo build -p ce-compiler` | Yes (implicit) |
| TGT-01 | `gen_skip_program` produces `Command::Skip` | Unit test: call generator, assert AST contains Skip | `cargo test -p ce-compiler gen_skip_program` | No — Wave 0 |
| TGT-02 | `gen_simple_if` produces `Command::If` with 1 guard | Unit test | `cargo test -p ce-compiler gen_simple_if` | No — Wave 0 |
| TGT-03 | `gen_multi_guard_if(2)` / `(3)` produce n-guard `Command::If` | Unit test | `cargo test -p ce-compiler gen_multi_guard_if` | No — Wave 0 |
| TGT-04 | `gen_simple_do` produces `Command::Loop` with 1 guard | Unit test | `cargo test -p ce-compiler gen_simple_do` | No — Wave 0 |
| TGT-05 | `gen_do_n_guards(n)` produces n-guard `Command::Loop` | Unit test | `cargo test -p ce-compiler gen_do_n_guards` | No — Wave 0 |
| TGT-06 | `gen_nested_if_in_do` contains `Command::If` inside a guard body of `Command::Loop` | Unit test with pattern match | `cargo test -p ce-compiler gen_nested_if_in_do` | No — Wave 0 |
| TGT-07 | `gen_nested_do_in_if` contains `Command::Loop` inside a guard body of `Command::If` | Unit test with pattern match | `cargo test -p ce-compiler gen_nested_do_in_if` | No — Wave 0 |
| TGT-08 | `gen_array_assignment` LHS is `Target::Array` | Unit test | `cargo test -p ce-compiler gen_array_assignment` | No — Wave 0 |
| TGT-09 | `gen_array_read` RHS is `AExpr::Reference(Target::Array(...))` | Unit test | `cargo test -p ce-compiler gen_array_read` | No — Wave 0 |
| TGT-10 | `gen_non_deterministic_overlapping` produces `Command::If` with 2+ guards | Unit test | `cargo test -p ce-compiler gen_non_deterministic_overlapping` | No — Wave 0 |
| TGT-11 | `gen_variable_reuse` produces guards with same variable in LHS | Unit test | `cargo test -p ce-compiler gen_variable_reuse` | No — Wave 0 |
| TGT-12 | `gen_variable_as_index` uses same variable name as array index and scalar | Unit test | `cargo test -p ce-compiler gen_variable_as_index` | No — Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p ce-compiler`
- **Per wave merge:** `cargo test`
- **Phase gate:** Full suite green before marking phase complete

### Wave 0 Gaps
All tests listed above are new. They should be added directly in `crates/envs/ce-compiler/src/lib.rs` (following the existing `point4_oracle_memory_states` test at line 159) or in a new `crates/envs/ce-compiler/src/tests.rs` module.

The test structure for each targeted generator follows this pattern:
```rust
#[test]
fn gen_skip_program_guarantees_skip() {
    use rand::SeedableRng;
    use ce_core::gn::compiler_gen::{gen_skip_program, CompilerContext};
    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    for _ in 0..50 {
        let cmds = gen_skip_program(&mut CompilerContext::default(), &mut rng);
        assert!(
            cmds.0.iter().any(|c| matches!(c, gcl::ast::Command::Skip)),
            "gen_skip_program must always contain Command::Skip"
        );
    }
}
```

For nested constructs (TGT-06, TGT-07), pattern-match into guard bodies:
```rust
fn contains_if_in_loop(cmds: &gcl::ast::Commands) -> bool {
    cmds.0.iter().any(|c| match c {
        gcl::ast::Command::Loop(guards) => {
            guards.iter().any(|gcl::ast::Guard(_, body)| {
                body.0.iter().any(|inner| matches!(inner, gcl::ast::Command::If(_)))
            })
        }
        _ => false,
    })
}
```

---

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| `gcl_gen.rs` trait impls with generic R | `gcl_compiler_gen.rs` free functions + ErasedRng bridge | The bridge exists because `dyn Fn(&mut Ctx, &mut R)` is not object-safe when R is generic |
| No Skip in `gcl_gen.rs` (no arm in Command impl) | Skip included in `gcl_compiler_gen.rs` at weight 0.3 | Phase 1 makes Skip GUARANTEED via catalog |
| `use_array()` hardcoded `false` in `gcl_gen.rs` | `gcl_compiler_gen.rs` uses `cx.use_array()` based on `no_arrays` | Phase 1 makes array constructs GUARANTEED via catalog |

---

## Open Questions

1. **Should `gen_commands` also receive a `Scenario` override parameter for testing?**
   - What we know: The current signature is `gen_commands<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands` and GEN-03 requires it unchanged.
   - What's unclear: For the Phase 3 statistical test (PRF-02), it may be useful to call a specific targeted generator directly by name. But that's not a change to `gen_commands` — PRF-02 can call each targeted generator directly.
   - Recommendation: Leave `gen_commands` unchanged. Tests call targeted generators directly by name.

2. **How many `Random` scenario slots in the catalog?**
   - What we know: Too many `Random` slots means the old bugs reappear at that frequency. Too few means the output is very structured and may not cover edge cases.
   - Recommendation: Start with `Random` at weight 3.0 out of ~22 total weight (~14% random). Adjust after Phase 3 statistics.

3. **Should targeted generators call `gen_command` for filler or generate assignments directly?**
   - What we know: `gen_command` can recurse into nested if/do, which may confuse what the "guaranteed" construct is for verification purposes.
   - Recommendation: For filler commands, use `Command::Assignment(gen_target(cx, rng), gen_aexpr(cx, rng))` directly rather than `gen_command`, to keep the structure predictable and tests simple.

---

## Sources

### Primary (HIGH confidence)
- `crates/ce-core/src/gn/gcl_compiler_gen.rs` — current generator sketch, read in full
- `crates/ce-core/src/gn/gcl_gen.rs` — baseline generator, read in full
- `crates/envs/ce-compiler/src/lib.rs` — caller code, read in full
- `crates/gcl/src/ast.rs` — GCL AST type definitions, read in full
- `crates/gcl/src/ast_ext.rs` — convenience constructors (`AExpr::binary`, `BExpr::logic`), read in full
- `crates/gcl/src/pg.rs` — program graph edge generation, read lines 1-226 (edge semantics for Loop, If, Skip, Assignment)
- `crates/ce-core/src/gn.rs` — module organization, read in full

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` — requirements cross-referenced for all TGT-* IDs
- `.planning/ROADMAP.md` — plan structure and phase success criteria

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, existing crate usage verified in source
- Architecture patterns: HIGH — synthesized from reading actual source code
- Pitfalls: HIGH — derived from reading the existing ErasedRng pattern, fuel mechanism, and caller constraints in real code
- AST constructors: HIGH — read from `ast.rs` and `ast_ext.rs` directly

**Research date:** 2026-03-22
**Valid until:** Stable until GCL AST types change — no fast-moving dependencies
