use gcl::ast::{
    AExpr, AOp, Array, BExpr, Command, Commands, Guard, LogicOp, RelOp, Target, Variable,
};
use rand::{
    Rng, SeedableRng,
    rngs::SmallRng,
    seq::{IndexedRandom, SliceRandom},
};

use crate::gn::compiler_gen::{CompilerContext, gen_aexpr, gen_bexpr, gen_target};
type ErasedRng = SmallRng;

type GenFn<G> = Box<dyn Fn(&mut CompilerContext, &mut ErasedRng) -> G>;
type GenOptions<G> = Vec<(f32, GenFn<G>)>;

type GenFnNested<G> = Box<dyn Fn(&mut CompilerContext, &mut ErasedRng, &GenOptionsNested<G>) -> G>;
pub struct GenOptionsNested<G>(pub Vec<(f32, GenFnNested<G>)>);

pub struct InterpreterContext {
    pub level: u32,
    pub compiler_context: CompilerContext,
}

impl Default for InterpreterContext {
    fn default() -> Self {
        Self {
            level: 1,
            compiler_context: CompilerContext::default(),
        }
    }
}

impl<G> GenOptionsNested<G> {
    pub fn generate(&self, cx: &mut CompilerContext, rng: &mut ErasedRng) -> G {
        cx.fuel = cx.fuel.checked_sub(1).unwrap_or_default();

        // TODO if there is no more fuel end generation with something from lvl_assignment

        let mut erng = SmallRng::seed_from_u64(rng.random());
        let (_, f) = &self.0.choose_weighted(&mut erng, |item| item.0).unwrap();

        f(cx, &mut erng, &self)
    }
}

pub fn generate_selective<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> Commands {
    let mut cmds: Vec<Command> = Vec::new();
    let mut generation_options: GenOptionsNested<Commands> = GenOptionsNested(vec![]);

    if cx.level <= 1 {
        // ? 1 Assignment: state updates (single assignments)

        // return the command immediately if it is level 1,
        // as the generator will normally always generate a sequence of at least 3 commands
        if cx.level == 1 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_assignment(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            return Commands(cmds);
        }

        generation_options
            .0
            .append(&mut lvl_assignment(&mut cx.compiler_context).0);
    }

    if cx.level >= 2 {
        // ? 2 Sequencing: multiple steps ( sequential composition C1 ; C2, always deterministic, no branching, Should always be common afterwards)
        if cx.level == 2 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_sequencing(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options
            .0
            .append(&mut lvl_sequencing(&mut cx.compiler_context).0);
    }

    if cx.level >= 3 {
        // ? 3 Conditionals: bool branching, execution depends on guards being true, always deterministic. For example: if b1 → C1 [] ... [] bn → Cn fi
        if cx.level == 3 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_conditionals()
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options.0.append(&mut lvl_conditionals().0);
    }

    if cx.level >= 4 {
        // ? 4 Stuck: unsolvable programs, guards are all false, or semantics undefined like division by zero
        if cx.level == 4 {
            // as this is before loops they have to be disabled
            cx.compiler_context.no_loops = false;

            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_stuck(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options
            .0
            .append(&mut lvl_stuck(&mut cx.compiler_context).0);
    }

    if cx.level >= 5 {
        // ? 5 Loops: long execution (execution that may surpass the trace length limit) do GC od introduces iteration, exits when no guards hold. This level will bring potentially infinite execution, and differences between terminated, running, stuck( we have in the code exactly as TerminationState::Running TerminationState::Terminated TerminationState::Stuck
        if cx.level == 5 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_loops(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options
            .0
            .append(&mut lvl_loops(&mut cx.compiler_context).0);
    }

    if cx.level >= 6 {
        // ? 6 Nondeterminism: multiple valid paths, overlapping guards in if / do (we have also implemented the new nondeterministic path for this one: nexts() choose_random(...)
        if cx.level == 6 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_nondeterminism(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options
            .0
            .append(&mut lvl_nondeterminism(&mut cx.compiler_context).0);
    }

    if cx.level >= 7 {
        // ? 7 Undefined semantics:
        if cx.level == 7 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_undefined(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        generation_options
            .0
            .append(&mut lvl_undefined(&mut cx.compiler_context).0);
    }

    if cx.level >= 8 {
        // ? 8 Composition: (all previous levels are guaranteed here)
        if cx.level == 8 {
            let mut erng = SmallRng::seed_from_u64(rng.random());
            cmds.append(
                &mut lvl_assignment(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_sequencing(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_conditionals()
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_stuck(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_loops(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_nondeterminism(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
            cmds.append(
                &mut lvl_undefined(&mut cx.compiler_context)
                    .generate(&mut cx.compiler_context, &mut erng)
                    .0,
            );
        }

        //generation_options.0.append(&mut lvl_composition().0);
    }

    let min = 3;
    let min: u32 = 0.max((min - cmds.len()).try_into().unwrap());
    let max = cx.compiler_context.fuel.max(min);
    let n = rng.random_range(min..=max);

    for i in 0..n {
        let mut erng = SmallRng::seed_from_u64(rng.random());
        cmds.append(
            &mut generation_options
                .generate(&mut cx.compiler_context, &mut erng)
                .0,
        );

        if cx.compiler_context.fuel <= 0 && i <= min {
            break;
        }
    }

    // so that the guaranteed additions do not always appear as the first value
    cmds.shuffle(rng);

    Commands(cmds)
}

// ? 1 Assignment: state updates (single assignments)
fn lvl_assignment(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    // as these commands are essentially the stopping points for the generator,
    // we modify the change for them to generate depending on the current fuel of the generation
    let point_of_no_return: Box<dyn Fn(&mut CompilerContext) -> f32> =
        Box::new(|cx: &mut CompilerContext| {
            if cx.fuel <= 0 {
                return 1000.0;
            } else if cx.fuel <= 1 {
                return 10.0;
            } else if cx.fuel <= 5 {
                return 2.0;
            } else {
                return 0.5;
            }
        });

    GenOptionsNested(vec![
        (
            point_of_no_return(cx),
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 _gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![gen_assignment(cx, rng)])
                },
            ),
        ),
        (
            point_of_no_return(cx),
            Box::new(
                |_cx: &mut CompilerContext,
                 _rng: &mut ErasedRng,
                 _gnopt: &GenOptionsNested<Commands>| Commands(vec![Command::Skip]),
            ),
        ),
    ])
}

// ? 2 Sequencing: multiple steps ( sequential composition C1 ; C2, always deterministic, no branching, Should always be guaranteed afterwards)
fn lvl_sequencing(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            if cx.fuel >= 2 { 2.0 } else { 0.0 },
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    let mut seq = gnopt.generate(cx, rng).0;
                    seq.append(&mut gnopt.generate(cx, rng).0);
                    Commands(seq)
                },
            ),
        ),
        (
            if cx.fuel >= 3 { 1.0 } else { 0.0 },
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    let mut seq = gnopt.generate(cx, rng).0;
                    seq.append(&mut gnopt.generate(cx, rng).0);
                    seq.append(&mut gnopt.generate(cx, rng).0);
                    Commands(seq)
                },
            ),
        ),
    ])
}

// ? 3 Conditionals: bool branching, execution depends on guards being true, always deterministic. For example: if b1 → C1 [] ... [] bn → Cn fi
fn lvl_conditionals() -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![(
        0.5,
        Box::new(
            |cx: &mut CompilerContext, rng: &mut ErasedRng, gnopt: &GenOptionsNested<Commands>| {
                Commands(vec![Command::If(vec![Guard(
                    gen_bexpr(cx, rng),
                    gnopt.generate(cx, rng),
                )])])
            },
        ),
    )])
}

// ? 4 Stuck: unsolvable programs, guards are all false
fn lvl_stuck(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.5,
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(vec![Guard(
                        gen_bexpr_stuck(cx, rng),
                        gnopt.generate(cx, rng),
                    )])])
                },
            ),
        ),
        (
            if cx.no_loops { 0.0 } else { 0.5 },
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(vec![Guard(
                        gen_bexpr_stuck(cx, rng),
                        gnopt.generate(cx, rng),
                    )])])
                },
            ),
        ),
    ])
}

// ? 5 Loops: long execution (execution that may surpass the trace length limit) do GC od introduces iteration, exits when no guards hold. This level will bring potentially infinite execution, and differences between terminated, running, stuck( we have in the code exactly as TerminationState::Running TerminationState::Terminated TerminationState::Stuck
fn lvl_loops(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![(
        if cx.no_loops { 0.0 } else { 0.5 },
        Box::new(
            |cx: &mut CompilerContext, rng: &mut ErasedRng, gnopt: &GenOptionsNested<Commands>| {
                Commands(vec![Command::Loop(vec![Guard(
                    gen_bexpr(cx, rng),
                    gnopt.generate(cx, rng),
                )])])
            },
        ),
    )])
}

// ? 6 Nondeterminism: multiple valid paths, overlapping guards in if / do (we have also implemented the new nondeterministic path for this one: nexts() choose_random(...)
fn lvl_nondeterminism(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.5,
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(gen_multiple_guards(cx, rng, gnopt))])
                },
            ),
        ),
        (
            if cx.no_loops { 0.0 } else { 0.5 },
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(gen_multiple_guards(cx, rng, gnopt))])
                },
            ),
        ),
    ])
}

// ? 7 Undefined semantics: division by zero
fn lvl_undefined(cx: &mut CompilerContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.5,
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    // TODO add undefined semantics, could be done similar to  Nondeterminism
                    // Condition
                    Commands(vec![Command::Skip])
                },
            ),
        ),
        (
            if cx.no_loops { 0.0 } else { 0.5 },
            Box::new(
                |cx: &mut CompilerContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    // TODO add undefined semantics, could be done similar to  Nondeterminism
                    // Loop
                    Commands(vec![Command::Skip])
                },
            ),
        ),
    ])
}

// ? 8 Composition: (all previous levels are guaranteed here)
// fn lvl_composition() -> GenOptionsNested<Commands> {
//     GenOptionsNested(vec![(
//         1.0,
//         Box::new(
//             |cx: &mut CompilerContext, rng: &mut ErasedRng, gnopt: &GenOptionsNested<Commands>| {
//                 Commands(vec![Command::Skip])
//             },
//         ),
//     )])
// }

// ? helper functions

fn gen_assignment<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> Command {
    Command::Assignment(gen_target(cx, rng), gen_aexpr(cx, rng))
}

pub fn gen_bexpr_stuck<R: Rng>(cx: &mut CompilerContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        (1.0, Box::new(|_, _| BExpr::Bool(false))),
        (
            if cx.negation_limit == 0 { 0.0 } else { 0.5 },
            Box::new(|cx: &mut CompilerContext, _| {
                cx.negation_limit = cx.negation_limit.checked_sub(1).unwrap_or_default();
                BExpr::Not(Box::new(BExpr::Bool(true)))
            }),
        ),
        (
            if cx.negation_limit == 0 { 0.0 } else { 0.5 },
            Box::new(|cx: &mut CompilerContext, rng: &mut ErasedRng| {
                cx.negation_limit = cx.negation_limit.checked_sub(1).unwrap_or_default();
                BExpr::Not(Box::new(BExpr::Not(Box::new(gen_bexpr_stuck(cx, rng)))))
            }),
        ),
        // TODO more generation options than true, false and NOT
        // missing: all LogicOps for BExpr::Logic and all RelOps for BExpr::Rel
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_multiple_guards(
    cx: &mut CompilerContext,
    rng: &mut ErasedRng,
    gnopt: &GenOptionsNested<Commands>,
) -> Vec<Guard> {
    let n = rng.random_range(0..cx.fuel.max(1));

    let guards: Vec<Guard> = (0..n)
        .map(|_| Guard(gen_bexpr(cx, rng), gnopt.generate(cx, rng)))
        .collect();

    guards
}
