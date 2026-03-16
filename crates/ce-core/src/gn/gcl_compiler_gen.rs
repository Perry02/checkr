/// Separate test-case generator for the compiler environment.
///
/// Unlike [`gcl_gen`], this generator enables:
/// - **Arrays** — randomly generates `A[i]`, `B[i]`, `C[i]` targets
/// - **`skip`** — emits `Command::Skip` at low probability
/// - **Unary minus** — emits `AExpr::Minus(e)` at moderate probability
///
/// All generation is done via plain `gen_*` free functions rather than
/// `Generate` trait impls, to avoid having duplicate trait impls on the same
/// GCL AST types.
use gcl::ast::{
    AExpr, AOp, Array, BExpr, Command, Commands, Guard, LogicOp, RelOp, Target, Variable,
};
use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IndexedRandom};

/// Internal concrete RNG type used for erased closure storage.
///
/// We use a concrete RNG type in closures so we can box them without hitting
/// the dyn-incompatibility of `rand::Rng` (which has generic methods).
/// Callers pass any `impl Rng`; we bridge via [`Ctx::sample`] which uses
/// generics at the call boundary.
type ErasedRng = SmallRng;

type GenFn<G> = Box<dyn Fn(&mut CompilerContext, &mut ErasedRng) -> G>;
type GenOptions<G> = Vec<(f32, GenFn<G>)>;

pub struct CompilerContext {
    pub fuel: u32,
    pub recursion_limit: u32,
    pub negation_limit: u32,
    pub no_loops: bool,
    pub no_division: bool,
    pub no_unary_minus: bool,
    pub no_arrays: bool,
    /// Scalar variable names used as assignment targets / references.
    pub names: Vec<String>,
    /// Array names used as array targets.
    pub array_names: Vec<String>,
}

impl Default for CompilerContext {
    fn default() -> Self {
        Self {
            fuel: 10,
            recursion_limit: Default::default(),
            negation_limit: Default::default(),
            no_loops: Default::default(),
            no_division: Default::default(),
            no_unary_minus: Default::default(),
            no_arrays: Default::default(),
            names: ["a", "b", "c", "d"].map(Into::into).to_vec(),
            array_names: ["A", "B", "C"].map(Into::into).to_vec(),
        }
    }
}

impl CompilerContext {
    pub fn new<R: Rng>(fuel: u32, _rng: &mut R) -> Self {
        CompilerContext {
            fuel,
            recursion_limit: fuel,
            negation_limit: fuel,
            ..Default::default()
        }
    }

    pub fn set_no_loop(&mut self, no_loops: bool) -> &mut Self {
        self.no_loops = no_loops;
        self
    }
    pub fn set_no_division(&mut self, no_division: bool) -> &mut Self {
        self.no_division = no_division;
        self
    }
    pub fn set_no_unary_minus(&mut self, no_unary_minus: bool) -> &mut Self {
        self.no_unary_minus = no_unary_minus;
        self
    }
    pub fn set_no_arrays(&mut self, no_arrays: bool) -> &mut Self {
        self.no_arrays = no_arrays;
        self
    }

    fn use_array(&self) -> bool {
        !self.no_arrays && !self.array_names.is_empty()
    }

    fn many<G, R: Rng>(
        &mut self,
        min: usize,
        max: usize,
        rng: &mut R,
        f: fn(&mut CompilerContext, &mut R) -> G,
    ) -> Vec<G> {
        let max = max.min(self.fuel as _).max(min);
        let n = rng.random_range(min..=max);
        if self.fuel < n as _ {
            self.fuel = 0;
        } else {
            self.fuel -= n as u32;
        }
        (0..n).map(|_| f(self, rng)).collect()
    }

    /// Like `many` but pinned to the internal `ErasedRng`, for use inside boxed closures.
    fn many_erased<G>(
        &mut self,
        min: usize,
        max: usize,
        rng: &mut ErasedRng,
        f: fn(&mut CompilerContext, &mut ErasedRng) -> G,
    ) -> Vec<G> {
        self.many(min, max, rng, f)
    }

    /// Sample one of the provided options by weight.
    ///
    /// We work around `rand::Rng`'s dyn-incompatibility by accepting a concrete
    /// `SmallRng` reference. The public `gen_*` functions accept any `R: Rng`
    /// and forward a reseeded `SmallRng` here.
    fn sample<G>(&mut self, rng: &mut ErasedRng, options: GenOptions<G>) -> G {
        let f = options.choose_weighted(rng, |o| o.0).unwrap();
        f.1(self, rng)
    }
}

// ---------------------------------------------------------------------------
// Helper: bridge any Rng to the internal ErasedRng
// ---------------------------------------------------------------------------

fn bridge<R: Rng>(rng: &mut R) -> ErasedRng {
    SmallRng::seed_from_u64(rng.random())
}

// ---------------------------------------------------------------------------
// Public generation entry points
// ---------------------------------------------------------------------------

pub fn gen_commands<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Commands {
    Commands(cx.many(1, 10, rng, gen_command))
}

pub fn gen_command<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Command {
    cx.recursion_limit = 5;
    cx.negation_limit = 3;
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                1.0,
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    Command::Assignment(gen_target(cx, rng), gen_aexpr(cx, rng))
                }),
            ),
            // skip is a real compiler edge — include it at a low weight
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| Command::Skip),
            ),
            (
                0.6,
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    Command::If(cx.many_erased(1, 10, rng, gen_command_erased))
                }),
            ),
            (
                if cx.no_loops { 0.0 } else { 0.3 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    Command::Loop(cx.many_erased(1, 10, rng, gen_command_erased))
                }),
            ),
        ],
    )
}

pub fn gen_target<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Target<Box<AExpr>> {
    gen_reference(cx, rng)
}

pub fn gen_guard<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Guard {
    cx.recursion_limit = 5;
    cx.negation_limit = 3;
    Guard(gen_bexpr(cx, rng), gen_commands(cx, rng))
}

pub fn gen_aexpr<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> AExpr {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                0.4,
                Box::new(|_cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    AExpr::Number(rng.random_range(-100..=100))
                }),
            ),
            (
                if cx.names.is_empty() && cx.array_names.is_empty() {
                    0.0
                } else {
                    0.8
                },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    AExpr::Reference(gen_reference(cx, rng))
                }),
            ),
            (
                if cx.recursion_limit == 0 || cx.fuel == 0 {
                    0.0
                } else {
                    0.9
                },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    cx.recursion_limit = cx.recursion_limit.checked_sub(1).unwrap_or_default();
                    AExpr::binary(gen_aexpr(cx, rng), gen_aop(cx, rng), gen_aexpr(cx, rng))
                }),
            ),
            // Unary minus: -expr
            (
                if cx.no_unary_minus || cx.recursion_limit == 0 || cx.fuel == 0 {
                    0.0
                } else {
                    0.4
                },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    cx.recursion_limit = cx.recursion_limit.checked_sub(1).unwrap_or_default();
                    AExpr::Minus(Box::new(gen_aexpr(cx, rng)))
                }),
            ),
        ],
    )
}

pub fn gen_aop<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> AOp {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                0.5,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| AOp::Plus),
            ),
            (
                0.4,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| AOp::Minus),
            ),
            (
                0.4,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| AOp::Times),
            ),
            (
                0.1,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| AOp::Pow),
            ),
            (
                if cx.no_division { 0.0 } else { 0.3 },
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| AOp::Divide),
            ),
        ],
    )
}

pub fn gen_bexpr<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> BExpr {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                0.2,
                Box::new(|_cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    BExpr::Bool(rng.random())
                }),
            ),
            (
                if cx.recursion_limit == 0 { 0.0 } else { 0.7 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    cx.recursion_limit = cx.recursion_limit.checked_sub(1).unwrap_or_default();
                    BExpr::Rel(gen_aexpr(cx, rng), gen_relop(cx, rng), gen_aexpr(cx, rng))
                }),
            ),
            (
                if cx.recursion_limit == 0 { 0.0 } else { 0.7 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    cx.recursion_limit = cx.recursion_limit.checked_sub(1).unwrap_or_default();
                    BExpr::logic(
                        gen_bexpr(cx, rng),
                        gen_logicop(cx, rng),
                        gen_bexpr(cx, rng),
                    )
                }),
            ),
            (
                if cx.negation_limit == 0 { 0.0 } else { 0.4 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    cx.negation_limit = cx.negation_limit.checked_sub(1).unwrap_or_default();
                    BExpr::Not(Box::new(gen_bexpr(cx, rng)))
                }),
            ),
        ],
    )
}

pub fn gen_relop<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> RelOp {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Eq),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Gt),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Ge),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Lt),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Le),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| RelOp::Ne),
            ),
        ],
    )
}

pub fn gen_logicop<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> LogicOp {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| LogicOp::And),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| LogicOp::Land),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| LogicOp::Or),
            ),
            (
                0.3,
                Box::new(|_cx: &mut CompilerContext, _rng: &mut ErasedRng| LogicOp::Lor),
            ),
        ],
    )
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Erased-RNG version of gen_command for use inside `many`.
fn gen_command_erased(cx: &mut CompilerContext, rng: &mut ErasedRng) -> Command {
    gen_command(cx, rng)
}

fn gen_reference<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Target<Box<AExpr>> {
    let mut erng = bridge(rng);
    cx.sample(
        &mut erng,
        vec![
            (
                if cx.names.is_empty() { 0.0 } else { 0.7 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    Target::Variable(Variable(cx.names.choose(rng).cloned().unwrap()))
                }),
            ),
            (
                if cx.use_array() { 0.3 } else { 0.0 },
                Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                    let name = cx
                        .array_names
                        .choose(rng)
                        .cloned()
                        .unwrap_or_else(|| "A".into());
                    Target::Array(Array(name), Box::new(gen_aexpr(cx, rng)))
                }),
            ),
        ],
    )
}
