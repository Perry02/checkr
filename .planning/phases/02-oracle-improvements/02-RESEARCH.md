# Phase 2: Oracle Improvements - Research

**Researched:** 2026-03-23
**Domain:** GCL program graph validation, co-generated witness memories, path-based fingerprinting
**Confidence:** HIGH

---

## Summary

Phase 2 fixes two verified weaknesses in the current `validate` implementation inside
`crates/envs/ce-compiler/src/lib.rs`:

**Weakness 1 — Fixed-seed memory sampling (ORC-01).** `validate` currently seeds
`SmallRng` from the constant `0xCEC34` and draws 10 memories to exercise the graph.
Because the seed never changes, every validation call tests the exact same 10 memory
states regardless of the program. A guard whose condition is never satisfied by those
10 states can be completely wrong in the student's graph and validation will still pass.
The fix is co-generation: when `gen_commands` produces a program it also produces
*witness memories* — one memory that makes each guard evaluate `true`, and one that
makes it evaluate `false`. These memories are attached to `Input` and forwarded to
`validate`, replacing the fixed-seed set.

**Weakness 2 — Action bag structural collision (ORC-02).** `action_bag` characterises
the graph by counting `(incoming-fingerprints, outgoing-fingerprints)` pairs per node.
This loses edge-order and path information. Two structurally different graphs that happen
to share the same multiset of node neighbourhoods pass as equal. The most important
failure case: a student who emits an `if` compiled with `Determinism::NonDeterministic`
rules instead of `Determinism::Deterministic` will produce a graph with the same action
multiset but different condition guards — the bag comparison misses this. Path-based
fingerprinting walks every path from `Start` and hashes the ordered action sequence on
that path. Paths that differ in condition expression (the `b ∧ ¬prev` vs plain `b`
difference between deterministic and non-deterministic compilation) will produce
different fingerprints.

**Primary recommendation:** Implement both changes as independent tasks.
Task 02-01 adds `witness_mems: Vec<InterpreterMemory>` to `Input`, generates witnesses
alongside the program, and uses them in `validate`. Task 02-02 introduces
`path_fingerprints` as a replacement or additional check that walks the `ParsedGraph`
from the start node via DFS/BFS, builds sorted canonical path-action sequences, and
compares them between the reference graph and the student graph.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| ORC-01 | Co-generation — alongside each program, produce witness memories that guarantee each guard evaluates both true and false | Witness generation design documented in §Architecture Patterns — Pattern 1 |
| ORC-02 | Replace or augment action bag validation with path-based fingerprinting — trace all paths from initial node, fingerprint edge label sequences, preventing structural collisions | Path fingerprint design documented in §Architecture Patterns — Pattern 2 |
</phase_requirements>

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `gcl` (internal) | workspace | ProgramGraph, Action, BExpr semantics evaluation | Only graph-walking API in the codebase |
| `rand` | 0.9.0 | RNG for witness generation | Already used throughout; `SmallRng::seed_from_u64` pattern established |
| `itertools` | workspace | `collect_vec`, `flatten` | Already used in `ce-compiler/src/lib.rs` |
| `petgraph` | 0.7.1 | Graph traversal (DFS/BFS from start node) | Already used in `ce-compiler` via `dot.rs` |
| `serde` + `tapi` | workspace | `Input` struct must cross the API boundary | All `Input` types carry these derives |

### No Additional Dependencies Needed

Both ORC-01 and ORC-02 are implementable with existing crate dependencies.
No new crates need to be added to `Cargo.toml`.

---

## Architecture Patterns

### Where Everything Lives

```
crates/envs/ce-compiler/src/
├── lib.rs        ← Input struct, Generate impl, validate(), action_bag(), fingerprint()
└── dot.rs        ← dot_to_petgraph()  (ParsedGraph, graph: petgraph::Graph<String, Action>)

crates/ce-core/src/gn/
└── gcl_compiler_gen.rs  ← gen_commands() entry point; all targeted generators
```

The `Input` struct (`lib.rs` lines 19-24) is the only API-crossing type that needs to
change. It currently holds `commands: Stringify<Commands>` and `determinism:
Determinism`. Adding `witness_mems: Vec<InterpreterMemory>` here satisfies ORC-01
success criterion 1 ("Input carries witness memories alongside the GCL program").

The `Generate for Input` impl (`lib.rs` lines 100-113) calls `gen_commands` and
constructs the `Input`. Witness generation logic must be called here, in `gn()`, after
`gen_commands` returns the `Commands`. The caller in `ce-compiler/src/lib.rs` (via
`define_env!` roundtrip test) must not change.

### Pattern 1: Witness Memory Co-generation (ORC-01)

**What:** For a given `Commands` AST, enumerate every guard expression (`BExpr`) that
appears in any `Command::If` or `Command::Loop`. For each guard, produce two
`InterpreterMemory` values:
- A *true-witness* where `bexpr.semantics(&mem) == Ok(true)`
- A *false-witness* where `bexpr.semantics(&mem) == Ok(false)`

**How to extract guard BExprs from Commands:**

Walk the AST recursively:
```rust
fn collect_guards(cmds: &Commands) -> Vec<BExpr> {
    let mut result = Vec::new();
    for cmd in &cmds.0 {
        match cmd {
            Command::If(guards) | Command::Loop(guards) => {
                for Guard(b, body) in guards {
                    result.push(b.clone());
                    result.extend(collect_guards(body));
                }
            }
            _ => {}
        }
    }
    result
}
```

**How to find a satisfying memory for a BExpr:**

The simplest deterministic approach:
1. Compute `commands.fv()` to get the free variables.
2. Try candidate memories by randomly sampling from `rng` (same as current oracle does).
3. Evaluate `bexpr.semantics(&mem)` — if it returns `Ok(true)`, that's the true-witness;
   if `Ok(false)`, that's the false-witness.
4. Retry with a fresh memory if neither case is satisfied on first draw (guards like
   `BExpr::Bool(true)` need no iteration; `BExpr::Bool(false)` can never be satisfied
   as true — see edge cases below).

**When witness cannot be found:** Some guards are tautologies (`BExpr::Bool(true)`) or
contradictions (`BExpr::Bool(false)`). In these cases one direction cannot be
satisfied:
- Store `None` for the unsatisfiable direction, or simply omit it from the witness
  list.
- `validate` should skip witness evaluation for `None` slots.

**Memory shape:** `InterpreterMemory` from `gcl::interpreter`:
```rust
pub struct InterpreterMemory {
    pub variables: BTreeMap<Variable, Int>,
    pub arrays: BTreeMap<Array, Vec<Int>>,
}
```
This is already `Serialize + Deserialize + tapi::Tapi` (line 13-20 of `interpreter.rs`),
so it can be stored in `Input` without any additional trait work.

**How validate uses witness memories:**

```rust
// In validate(), replace the fixed-seed block:
// OLD:
//   let mut rng = SmallRng::seed_from_u64(0xCEC34);
//   let sample_mems = (0..10).map(|_| { ... }).collect_vec();
//
// NEW:
let sample_mems = &input.witness_mems;
// Rest of validate() unchanged.
```

The `action_bag` function signature accepts `&[InterpreterMemory]`, so it works with
both old and new memory sets without modification.

**When `witness_mems` is empty (student supplies raw Input without witnesses):**
Fall back to the fixed-seed sampling so the API remains usable from the UI without
requiring generated inputs.

### Pattern 2: Path-Based Fingerprinting (ORC-02)

**What:** Walk every path from `Node::Start` (identified as `"qStart"` in the
`ParsedGraph`) using DFS. For each complete path (reaching a sink node or a visited
cycle boundary), record the ordered sequence of `Action` labels as a `Vec<String>`.
Collect these into a `BTreeSet<Vec<String>>` (canonical, order-independent across
paths, order-preserving within a path). Compare the reference graph's path set to the
student graph's path set.

**Why this catches determinism errors:** In non-deterministic compilation, a guard
`b₁` in an `if` statement produces an edge labelled `"b₁"`. In deterministic
compilation, the same guard produces `"b₁ ∧ ¬false"` on the first guard and
`"b₂ ∧ ¬b₁"` on the second. These are textually different action labels — path
fingerprints will differ even if the node count and action type bag are equal.

**DFS path traversal on `ParsedGraph`:**

```
// Source: petgraph traversal on ParsedGraph.graph
// ParsedGraph.graph: petgraph::Graph<String, gcl::pg::Action>
// Start node: the petgraph NodeIndex whose String label == "qStart"

fn path_fingerprints(g: &ParsedGraph) -> BTreeSet<Vec<String>> {
    let start = g.node_mapping.get("qStart")?;  // Option — missing = error
    let mut all_paths = BTreeSet::new();
    let mut stack: Vec<(NodeIndex, Vec<String>, BTreeSet<NodeIndex>)> = vec![
        (*start, vec![], BTreeSet::new()),
    ];
    while let Some((node, path, visited)) = stack.pop() {
        let outgoing: Vec<_> = g.graph.edges(node).collect();
        if outgoing.is_empty() {
            all_paths.insert(path);
        } else {
            for edge in outgoing {
                if !visited.contains(&edge.target()) {
                    let mut new_path = path.clone();
                    new_path.push(edge.weight().to_string());
                    let mut new_visited = visited.clone();
                    new_visited.insert(node);
                    stack.push((edge.target(), new_path, new_visited));
                }
            }
        }
    }
    all_paths
}
```

**Cycle handling:** GCL `do` loops create back-edges to the loop entry. Tracking
`visited` nodes per path prevents infinite loops. A path terminates when it reaches
a visited node (the cycle boundary) or a sink with no outgoing edges.

**Integration into `validate`:**

```rust
// In validate(), after or instead of action_bag comparison:
let o_paths = path_fingerprints(&o_g);
let t_paths = path_fingerprints(&t_g);
if o_paths != t_paths {
    return Ok(ValidationResult::Mismatch {
        reason: "path fingerprints differ".to_string(),
    });
}
```

**Augment vs replace:** Start by augmenting (run both checks, report the first
mismatch). This is safer because action_bag catches fast for simple cases; path
fingerprints catch structural determinism errors. If path fingerprints prove to be
a strict superset of what action_bag catches, action_bag can be removed in a follow-up.

### Anti-Patterns to Avoid

- **Changing `Generate for Input`'s Context type.** It is currently `type Context = ()`.
  The `define_env!` macro calls `gn(&mut (), &mut rng)`. Changing the context type would
  break the `env_roundtrip` test and the shell's input generation.
  Instead: call witness generation inside `gn()` using the already-available `rng`.

- **Storing `Commands` in `Input` alongside `Stringify<Commands>`.** `Input` stores
  `commands: Stringify<Commands>`. The `Stringify` type can be parsed with
  `try_parse()`. Witnesses must be generated from the `Commands` AST but stored as
  `Vec<InterpreterMemory>` — not by embedding the raw `Commands`.

- **Using `petgraph::algo::all_simple_paths` naively.** This function does not exist
  with that exact name in petgraph 0.7.x's public API surface. Use manual DFS stack
  as shown above. (Confidence: MEDIUM — petgraph docs not fetched, based on codebase
  usage patterns.)

- **Using path-string representation that includes node IDs.** Node IDs in
  `ParsedGraph` are strings like `"qStart"`, `"q1"`, `"qFinal"`. These differ between
  reference and student graph if the student uses different node naming. Fingerprints
  must be built from **edge action labels only**, not node names.

- **Adding `#[serde(default)]` without `#[serde(skip_serializing_if)]`.** If
  `witness_mems` is empty for old inputs (deserialized from JSON without the field),
  `serde(default)` on the field gives an empty `Vec`, which is correct. The field
  should use `#[serde(default)]` to remain backward-compatible with inputs that do not
  include witnesses.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Boolean semantics evaluation | Custom evaluator | `BExpr::semantics(&mem)` from `gcl::semantics` | Already handles all `BExpr` variants including `Not`, `Logic`, `Rel`, `Bool` |
| Memory construction from free variables | Custom `BTreeMap` builder | `gcl::memory::Memory::from_targets_with(commands.fv(), ...)` | Handles both `Variable` and `Array` targets; same pattern used in existing oracle |
| Graph traversal | petgraph wrapper | Direct use of `g.graph.edges(node)` and `g.node_mapping` via `ParsedGraph` | `ParsedGraph` already exposes petgraph internals |
| Action label serialisation | Custom `Display` | `action.to_string()` — `Action` implements `Display` via `gcl/src/pg.rs` line 130-138 | Output is `"x := expr"`, `"skip"`, or `"bexpr"` — canonical and parseable |

---

## Common Pitfalls

### Pitfall 1: Fixed-Seed Oracle Still Active After Partial Migration
**What goes wrong:** `witness_mems` is added to `Input` but `validate` still calls
`SmallRng::seed_from_u64(0xCEC34)` unconditionally. The new field is populated but
never used.
**Why it happens:** The old sampling code is 8 lines and not obviously dead after
`witness_mems` is wired in.
**How to avoid:** Replace — not augment — the sampling block in `validate`. Use
`if input.witness_mems.is_empty()` as the only escape hatch for legacy inputs.
**Warning signs:** `env_roundtrip` test passes with `validate` never touching the
new field.

### Pitfall 2: Witness Generation Only for Top-Level Guards
**What goes wrong:** `collect_guards` walks only the top-level `Commands`, missing
guards nested inside `do` bodies or `if` bodies.
**Why it happens:** GCL nests `if` and `do` arbitrarily deep; a non-recursive walk
misses inner guards.
**How to avoid:** Implement `collect_guards` as a recursive function, as shown above.
**Warning signs:** A `gen_nested_if_in_do` program produces witnesses that do not
cover the inner `if` guard.

### Pitfall 3: Path Explosion on Complex Programs
**What goes wrong:** Programs with many overlapping guards produce a combinatorial
explosion of distinct paths. Memory and time grow exponentially.
**Why it happens:** Each `if`/`do` branch doubles the path count. A 4-guard `do`
loop with 3 iterations has `4^3 = 64` paths before cycle cutoff.
**How to avoid:** Apply a path depth limit (e.g., `max_path_length = 2 * node_count`)
and a total path count limit (e.g., `max_paths = 1024`). Truncate gracefully and fall
back to `action_bag` comparison if limits are exceeded.
**Warning signs:** `env_roundtrip` test times out on programs with deeply nested loops.

### Pitfall 4: `tapi` Schema Change Breaks Auto-Generated F# Types
**What goes wrong:** Adding `witness_mems: Vec<InterpreterMemory>` to `Input` changes
the `tapi`-generated `Io.fs` and `api.ts`. If the field lacks `#[serde(default)]`, old
inputs (from the UI or from tests) fail to deserialize.
**Why it happens:** `tapi` generates TypeScript and F# types on server restart. Any
`Input` change propagates immediately.
**How to avoid:** Add `#[serde(default)]` to `witness_mems`. Mark the field as
optional in the UI layer (or default to hiding it). The F# student starter does not
need to populate this field.
**Warning signs:** Deserialization error when the UI sends `{"commands": "...",
"determinism": "Deterministic"}` without the new field.

### Pitfall 5: `Node::Start` String Name Assumption
**What goes wrong:** Path traversal starts from the node labelled `"qStart"` in
`ParsedGraph.node_mapping`. If the student's DOT output uses a different label for
the start node (e.g., `"q0"` or `"start"`), `path_fingerprints` returns an empty set
and the comparison trivially passes (both sides empty).
**Why it happens:** `dot_to_petgraph` is a generic DOT parser that does not assert
any convention about start-node naming.
**How to avoid:** If `node_mapping.get("qStart")` returns `None`, return a
`Mismatch` result rather than an empty set. Also consider that the reference graph
(`o_g`) is always generated by `ProgramGraph::dot()` which writes `"qStart"` as the
label for `Node::Start` (confirmed in `pg.rs` line 270-272 — `{a:?}` debug format of
`Node::Start` is `"qStart"`). The student's graph may not follow this convention.
**Warning signs:** A student submits a graph with a renamed start node and it passes
validation.

---

## Code Examples

Verified patterns from codebase source:

### Constructing a Memory from Free Variables (existing oracle pattern)
```rust
// Source: crates/envs/ce-compiler/src/lib.rs lines 61-78
let mut rng = <rand::rngs::SmallRng as rand::SeedableRng>::seed_from_u64(0xCEC34);
let initial_memory = gcl::memory::Memory::from_targets_with(
    commands.fv(),
    &mut rng,
    |rng, _| rng.random_range(-10..=10),
    |rng, _| {
        let len = rng.random_range(5..=10);
        (0..len).map(|_| rng.random_range(-10..=10)).collect()
    },
);
let mem = InterpreterMemory {
    variables: initial_memory.variables,
    arrays: initial_memory.arrays,
};
```

### Evaluating a BExpr Against a Memory
```rust
// Source: crates/gcl/src/semantics.rs lines 128-136
// BExpr::semantics returns Ok(bool) or Err(SemanticsError)
let result: Result<bool, SemanticsError> = bexpr.semantics(&mem);
```

### Evaluating an Action's Effect (used in fingerprint)
```rust
// Source: crates/envs/ce-compiler/src/lib.rs line 138
// Action::semantics returns Ok(InterpreterMemory) or Err(SemanticsError)
let result: Option<InterpreterMemory> = action.semantics(mem).ok();
```

### Action Display (for path labels)
```rust
// Source: crates/gcl/src/pg.rs lines 130-138
// impl Display for Action:
//   Assignment(v, x) => "{v} := {x}"
//   Skip => "skip"
//   Condition(b) => "{b}"
let label = action.to_string();
```

### Accessing Petgraph Edges on ParsedGraph
```rust
// Source: crates/envs/ce-compiler/src/lib.rs, dot.rs
// ParsedGraph.graph: petgraph::Graph<String, gcl::pg::Action>
// Outgoing edges from a node:
for edge in parsed_graph.graph.edges(node_index) {
    let target: petgraph::graph::NodeIndex = edge.target();
    let action: &gcl::pg::Action = edge.weight();
}
```

### Input Struct Pattern with serde(default) for backward compat
```rust
// Convention from codebase derive patterns (CONVENTIONS.md):
#[derive(tapi::Tapi, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[tapi(path = "Compiler")]
pub struct Input {
    pub commands: Stringify<Commands>,
    pub determinism: Determinism,
    #[serde(default)]
    pub witness_mems: Vec<InterpreterMemory>,
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed-seed 10 samples | Co-generated witnesses | Phase 2 | Validation exercises the actual guards that appear in this program |
| Action bag comparison | Path fingerprinting | Phase 2 | Catches determinism vs non-determinism structural errors |

---

## Open Questions

1. **Witness generation for tautology guards (`BExpr::Bool(true)`) and contradiction
   guards (`BExpr::Bool(false)`)**
   - What we know: `semantics` evaluates these immediately with no memory access.
     A `BExpr::Bool(true)` can never produce a false-witness.
   - What's unclear: Should the generator skip these programs? Or include one-sided
     witnesses?
   - Recommendation: Tolerate one-sided witnesses. The `NonDeterministicOverlapping`
     scenario (`gen_non_deterministic_overlapping` in `gcl_compiler_gen.rs`) uses
     `a > 0` and `a < 10` — both are satisfiable in both directions with integer ranges.
     For `BExpr::Bool(true/false)` produce no witness in the unsatisfiable direction;
     `validate` skips these.

2. **Maximum number of path-fingerprint paths before combinatorial explosion**
   - What we know: Programs with 4-guard `do` loops can generate many paths; the DFS
     visits each path once but the result set can be large.
   - What's unclear: At what program size does path enumeration time cross a
     validation time budget (the system has no explicit timeout for `validate()`).
   - Recommendation: Add a `MAX_PATHS = 512` constant. If the reference graph exceeds
     this, fall back to `action_bag` only. This is consistent with how complex inputs
     are handled elsewhere (timeout fallback exists as `ValidationResult::TimeOut`).

3. **Whether `petgraph::algo::all_simple_paths` is available in petgraph 0.7.1**
   - What we know: petgraph 0.7.1 is used in the workspace. The function exists in
     the petgraph docs for some versions.
   - What's unclear: Exact API signature and whether it handles `DiGraph` correctly.
   - Recommendation: Use manual DFS stack (as documented above) to avoid version
     uncertainty. This is a 20-line implementation and avoids an undiscovered API gap.
     Confidence: MEDIUM.

---

## Sources

### Primary (HIGH confidence)
- `crates/envs/ce-compiler/src/lib.rs` — complete source of current oracle, `action_bag`,
  `fingerprint`, `Generate for Input`; read in full
- `crates/gcl/src/pg.rs` — `ProgramGraph`, `Action`, `Edge`, `Node`, `Determinism`,
  `guard_edges` (deterministic vs non-deterministic compilation difference); read in full
- `crates/gcl/src/semantics.rs` — `BExpr::semantics`, `Action::semantics`; read in full
- `crates/gcl/src/memory.rs` — `Memory::from_targets_with`; read in full
- `crates/gcl/src/interpreter.rs` — `InterpreterMemory` struct; read (partial)
- `crates/envs/ce-compiler/src/dot.rs` — `ParsedGraph`, `dot_to_petgraph`; read in full
- `crates/ce-core/src/gn/gcl_compiler_gen.rs` — all targeted generators; read in full
- `crates/ce-core/src/lib.rs` — `Env` trait, `Generate` trait, `define_env!` macro; read in full
- `.planning/REQUIREMENTS.md` — ORC-01, ORC-02 exact wording
- `.planning/ROADMAP.md` — Phase 2 success criteria

### Secondary (MEDIUM confidence)
- `.planning/codebase/ARCHITECTURE.md` — layer structure, `Env` trait contract
- `.planning/codebase/CONVENTIONS.md` — `serde(derive)` patterns, `tapi` usage
- `.planning/codebase/TESTING.md` — `define_env!` roundtrip test, test runner (`cargo nextest`)

---

## Metadata

**Confidence breakdown:**
- ORC-01 witness co-generation design: HIGH — entire call chain read from source
- ORC-02 path fingerprinting design: HIGH for design intent; MEDIUM for petgraph API
  details (manual DFS avoids API uncertainty)
- Backward-compat serde pattern: HIGH — established project pattern

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable domain; only risk is petgraph API detail)
