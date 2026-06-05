use std::collections::BTreeMap;

use gcl::{
    ast::{AExpr, AOp, Array, BExpr, Command, Commands, Guard, LogicOp, RelOp, Target, Variable},
    interpreter::InterpreterMemory,
};
use rand::{
    Rng, SeedableRng,
    rngs::SmallRng,
    seq::{IndexedRandom, SliceRandom},
};
use std::option::Option;

use crate::gn::compiler_gen::{CompilerContext, gen_aexpr, gen_aop, gen_bexpr, gen_guard};
type ErasedRng = SmallRng;

type GenFn<G> = Box<dyn Fn(&mut InterpreterContext, &mut ErasedRng) -> G>;
type GenOptions<G> = Vec<(f32, GenFn<G>)>;

type GenFnNested<Command> =
    Box<dyn Fn(&mut InterpreterContext, &mut ErasedRng, &GenOptionsNested<Command>) -> Command>;

pub struct GenOptionsNested<Command>(pub Vec<(f32, GenFnNested<Command>)>);

pub struct InterpreterContext {
    pub level: u32,
    pub guarantee: bool,
    pub guarantee_gen: Option<GenOptionsNested<Commands>>,
    pub memory: InterpreterMemory,
    pub allow_loops: bool,
    pub multiple_guards: bool,
    pub compiler_context: CompilerContext,
}

impl Default for InterpreterContext {
    fn default() -> Self {
        Self {
            level: 1,
            guarantee: false,
            guarantee_gen: None,
            memory: InterpreterMemory {
                variables: BTreeMap::new(),
                arrays: BTreeMap::new(),
            },
            allow_loops: true,
            multiple_guards: true,
            compiler_context: CompilerContext::default(),
        }
    }
}

impl InterpreterContext {
    pub fn new<R: Rng>(level: u32, compiler_context: CompilerContext, _rng: &mut R) -> Self {
        InterpreterContext {
            level,
            compiler_context,
            ..Default::default()
        }
    }
}

impl GenOptionsNested<Commands> {
    pub fn generate(&self, cx: &mut InterpreterContext, rng: &mut ErasedRng) -> Commands {
        let com_cx = &mut cx.compiler_context;

        com_cx.fuel = com_cx.fuel.checked_sub(1).unwrap_or_default();

        let mut erng = SmallRng::seed_from_u64(rng.random());

        if cx.guarantee && cx.guarantee_gen.is_some() {
            {
                let guar_gen = cx.guarantee_gen.take().unwrap();

                let (_i, f) = &guar_gen
                    .0
                    .choose_weighted(&mut erng, |item| item.0)
                    .unwrap();

                let _i = _i + 1.0;

                cx.guarantee = false;

                return f(cx, &mut erng, self);
            };
        } else if com_cx.fuel == 0 || com_cx.fuel < erng.random_range(..5) {
            return Commands(vec![Command::Skip]);
        }

        let (_i, f) = &self.0.choose_weighted(&mut erng, |item| item.0).unwrap();

        let _i = _i + 1.0;

        f(cx, &mut erng, &self)
    }
}

pub fn generate_selective<R: Rng>(
    cx: &mut InterpreterContext,
    mut rng: &mut R,
) -> (Commands, InterpreterMemory) {
    let mut cmds: Vec<Command> = Vec::new();
    let mut generation_options: GenOptionsNested<Commands> = GenOptionsNested(vec![]);

    // limit guard length
    cx.compiler_context.negation_limit = 4.min(cx.compiler_context.fuel);
    cx.compiler_context.recursion_limit = 4.min(cx.compiler_context.fuel);

    // set inittial memeory:
    let mut tmp_commands_var_names_list: Vec<Command> = cx
        .compiler_context
        .names
        .iter()
        .map(|var_name| {
            Command::Assignment(
                Target::Variable(Variable(var_name.to_string())),
                AExpr::Number(0),
            )
        })
        .collect();

    let mut tmp_commands_var_array_list: Vec<Command> = cx
        .compiler_context
        .array_names
        .iter()
        .map(|arr_name| {
            Command::Assignment(
                Target::Array(Array(arr_name.to_string()), Box::new(AExpr::Number(0))),
                AExpr::Reference(Target::Array(
                    Array(arr_name.to_string()),
                    Box::new(AExpr::Number(0)),
                )),
            )
        })
        .collect();

    let mut complete_command_name_list: Vec<Command> = Vec::new();
    complete_command_name_list.append(&mut tmp_commands_var_names_list);
    complete_command_name_list.append(&mut tmp_commands_var_array_list);

    let initial_memory = gcl::memory::Memory::from_targets_with(
        Commands(complete_command_name_list).fv(),
        &mut rng,
        |rng, _| rng.random_range(-20..=20),
        |rng, _| {
            let len = rng.random_range(5..=10);
            (0..len).map(|_| rng.random_range(-20..=20)).collect()
        },
    );

    cx.memory = InterpreterMemory {
        variables: initial_memory.variables,
        arrays: initial_memory.arrays,
    };

    // ? 1 Assignment: state updates (single assignments)

    if cx.level >= 1 {
        if cx.level == 1 {
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_assignment());
            cx.compiler_context.fuel = 1;
        }

        generation_options.0.append(&mut lvl_assignment().0);
    }

    if cx.level >= 2 {
        // ? 2 Sequencing: multiple steps ( sequential composition C1 ; C2, always deterministic, no branching, Should always be common afterwards)
        if cx.level == 2 {
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_sequencing(cx));
        }

        generation_options.0.append(&mut lvl_sequencing(cx).0);
    }

    if cx.level >= 3 {
        // ? 3 Conditionals: bool branching, execution depends on guards being true, always deterministic. For example: if b1 → C1 [] ... [] bn → Cn fi
        if cx.level == 3 {
            cx.guarantee = true;
            cx.multiple_guards = false;
            cx.guarantee_gen = Some(lvl_conditionals(cx));
        }

        generation_options.0.append(&mut lvl_conditionals(cx).0);
    }

    if cx.level >= 4 {
        // ? 4 Stuck: unsolvable programs, guards are all false, or semantics undefined like division by zero
        if cx.level == 4 {
            // as this is before loops they have to be disabled
            cx.allow_loops = false;

            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_stuck(cx));
        }

        generation_options.0.append(&mut lvl_stuck(cx).0);
    }

    if cx.level >= 5 {
        // ? 5 Loops: long execution (execution that may surpass the trace length limit) do GC od introduces iteration, exits when no guards hold. This level will bring potentially infinite execution, and differences between terminated, running, stuck( we have in the code exactly as TerminationState::Running TerminationState::Terminated TerminationState::Stuck
        if cx.level == 5 {
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_loops());
        }

        generation_options.0.append(&mut lvl_loops().0);
    }

    if cx.level >= 6 {
        // ? 6 Nondeterminism: multiple valid paths, overlapping guards in if / do (we have also implemented the new nondeterministic path for this one: nexts() choose_random(...)
        if cx.level == 6 {
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_nondeterminism());
        }

        generation_options.0.append(&mut lvl_nondeterminism().0);
    }

    if cx.level >= 7 {
        // ? 7 Undefined semantics:
        if cx.level == 7 {
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_undefined());
        }

        generation_options.0.append(&mut lvl_undefined().0);
    }

    if cx.level >= 8 {
        // ? 8 Composition: (all previous levels are guaranteed here)
        if cx.level == 8 {
            let mut erng = SmallRng::seed_from_u64(rng.random());

            // level 1
            cx.compiler_context.fuel = 3;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_assignment());
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);

            // level 2
            cx.compiler_context.fuel = 3;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_sequencing(cx));
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);

            // level 3
            cx.compiler_context.fuel = 3;
            cx.multiple_guards = false;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_conditionals(cx));
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);
            cx.multiple_guards = true;

            // level 4
            cx.compiler_context.fuel = 3;
            cx.allow_loops = false;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_stuck(cx));
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);
            cx.allow_loops = true;

            // level 5
            cx.compiler_context.fuel = 3;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_loops());
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);

            // level 6
            cx.compiler_context.fuel = 3;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_nondeterminism());
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);

            // level 7
            cx.compiler_context.fuel = 3;
            cx.guarantee = true;
            cx.guarantee_gen = Some(lvl_undefined());
            cmds.append(&mut generation_options.generate(cx, &mut erng).0);
        }

        //generation_options.0.append(&mut lvl_composition().0);
    }

    let min: usize = 3;
    let min: u32 = {
        if min >= cmds.len() {
            0.max((min - cmds.len()).try_into().unwrap())
        } else {
            0.max((cmds.len() - min).try_into().unwrap())
        }
    };

    let max = cx.compiler_context.fuel.max(min);
    let n = rng.random_range(min..=max);

    for i in 0..n {
        let mut erng = SmallRng::seed_from_u64(rng.random());
        cmds.append(&mut generation_options.generate(cx, &mut erng).0);

        if cx.compiler_context.fuel == 0 && i <= min {
            break;
        }
    }

    // so that the guaranteed additions do not always appear as the first value
    cmds.shuffle(rng);

    (Commands(cmds), cx.memory.clone())
}

// ? 1 Assignment: state updates (single assignments)
fn lvl_assignment() -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![(
        1.0,
        Box::new(
            |cx: &mut InterpreterContext,
             rng: &mut ErasedRng,
             _gnopt: &GenOptionsNested<Commands>| {
                Commands(vec![gen_assignment(cx, rng)])
            },
        ),
    )])
}

// ? 2 Sequencing: multiple steps ( sequential composition C1 ; C2, always deterministic, no branching, Should always be guaranteed afterwards)
fn lvl_sequencing(cx: &mut InterpreterContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            2.0,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    cx.compiler_context.recursion_limit = cx
                        .compiler_context
                        .recursion_limit
                        .checked_sub(2)
                        .unwrap_or_default();

                    let mut seq = gnopt.generate(cx, rng).0;
                    seq.append(&mut gnopt.generate(cx, rng).0);
                    Commands(seq)
                },
            ),
        ),
        (
            if cx.compiler_context.fuel >= 3 {
                0.1
            } else {
                0.0
            },
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    cx.compiler_context.recursion_limit = cx
                        .compiler_context
                        .recursion_limit
                        .checked_sub(3)
                        .unwrap_or_default();

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
fn lvl_conditionals(cx: &mut InterpreterContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            if cx.multiple_guards { 0.5 } else { 1.0 },
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(vec![Guard(
                        gen_bexpr_nondet(cx, rng),
                        gnopt.generate(cx, rng),
                    )])])
                },
            ),
        ),
        (
            if cx.multiple_guards { 0.5 } else { 0.0 },
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(gen_multiple_guards(cx, rng, gnopt))])
                },
            ),
        ),
    ])
}

// ? 4 Stuck: unsolvable programs, guards are all false
fn lvl_stuck(cx: &mut InterpreterContext) -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.5,
            Box::new(
                |cx: &mut InterpreterContext,
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
            if cx.allow_loops { 0.5 } else { 0.0 },
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(vec![Guard(
                        gen_bexpr_stuck(cx, rng),
                        gnopt.generate(cx, rng),
                    )])])
                },
            ),
        ),
    ])
}

// ? 5 Loops: long execution (execution that may surpass the trace length limit) do GC od introduces iteration, exits when no guards hold. This level will bring potentially infinite execution, and differences between terminated, running, stuck( we have in the code exactly as TerminationState::Running TerminationState::Terminated TerminationState::Stuck
fn lvl_loops() -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.25,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(vec![Guard(
                        gen_bexpr_nondet(cx, rng),
                        gnopt.generate(cx, rng),
                    )])])
                },
            ),
        ),
        (
            0.25,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 _gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(vec![gen_guard(
                        &mut cx.compiler_context,
                        rng,
                    )])])
                },
            ),
        ),
    ])
}

// ? 6 Nondeterminism: multiple valid paths, overlapping guards in if / do (we have also implemented the new nondeterministic path for this one: nexts() choose_random(...)
fn lvl_nondeterminism() -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.8,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(gen_multiple_guards(cx, rng, gnopt))])
                },
            ),
        ),
        (
            0.2,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(gen_multiple_guards(cx, rng, gnopt))])
                },
            ),
        ),
    ])
}

// ? 7 Undefined semantics: division by zero
fn lvl_undefined() -> GenOptionsNested<Commands> {
    GenOptionsNested(vec![
        (
            0.4,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::If(gen_undefined_guards(cx, rng, gnopt))])
                },
            ),
        ),
        (
            0.2,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 gnopt: &GenOptionsNested<Commands>| {
                    Commands(vec![Command::Loop(gen_undefined_guards(cx, rng, gnopt))])
                },
            ),
        ),
        (
            0.4,
            Box::new(
                |cx: &mut InterpreterContext,
                 rng: &mut ErasedRng,
                 _gnopt: &GenOptionsNested<Commands>| {
                    let name = cx
                        .compiler_context
                        .array_names
                        .choose(rng)
                        .cloned()
                        .unwrap_or_else(|| "A".into());
                    let arr = Target::Array(
                        Array(name),
                        Box::new(AExpr::Number(rng.random_range(-100..=-1))),
                    );

                    Commands(vec![Command::Assignment(
                        arr,
                        gen_aexpr(&mut cx.compiler_context, rng),
                    )])
                },
            ),
        ),
    ])
}

// ? helper functions

fn gen_assignment<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> Command {
    Command::Assignment(gen_reference_defined(cx, rng), gen_aexpr_defined(cx, rng))
}

fn gen_reference_defined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> Target<Box<AExpr>> {
    let generation_options: GenOptions<Target<Box<AExpr>>> = vec![
        (
            if cx.compiler_context.names.is_empty() {
                0.0
            } else {
                0.7
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                let var_name = cx.compiler_context.names.choose(rng).cloned().unwrap();

                Target::Variable(Variable(var_name))
            }),
        ),
        (
            if cx.compiler_context.no_arrays {
                0.0
            } else {
                0.3
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                let name = cx
                    .compiler_context
                    .array_names
                    .choose(rng)
                    .cloned()
                    .unwrap_or_else(|| "A".into());

                let mut aexpr = gen_aexpr_defined(cx, rng);
                let (small, large, ex) = aexpr_resolve(aexpr.clone(), cx);

                if large < 0 {
                    aexpr = AExpr::Minus(Box::new(aexpr));
                }
                if small < 0 {
                    if rng.random_bool(0.5) {
                        aexpr = AExpr::Binary(
                            Box::new(aexpr),
                            AOp::Plus,
                            Box::new(AExpr::Number(small)),
                        );
                    } else {
                        aexpr = AExpr::Binary(
                            Box::new(AExpr::Number(-small)),
                            AOp::Plus,
                            Box::new(aexpr),
                        );
                    }
                }

                // brute force fix as references are not carried over
                if large > 5 {
                    aexpr = AExpr::Binary(
                        Box::new(aexpr),
                        AOp::Minus,
                        Box::new(AExpr::Number(large - rng.random_range(0..=5))),
                    );
                }

                if ex {
                    aexpr = AExpr::Number(rng.random_range(1..=5));
                }

                Target::Array(Array(name), Box::new(aexpr))
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: Target<Box<AExpr>> = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_aexpr_defined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> AExpr {
    let generation_options: GenOptions<AExpr> = vec![
        (
            0.4,
            Box::new(|_cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                AExpr::Number(rng.random_range(-100..=100))
            }),
        ),
        (
            if cx.compiler_context.names.is_empty() && cx.compiler_context.array_names.is_empty() {
                0.0
            } else {
                0.8
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                AExpr::Reference(gen_reference_defined(cx, rng))
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.9
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::binary(
                    gen_aexpr_defined(cx, rng),
                    gen_aop(&mut cx.compiler_context, rng),
                    gen_aexpr_defined(cx, rng),
                )
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.4
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::Minus(Box::new(gen_aexpr_defined(cx, rng)))
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_aexpr_simple<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> AExpr {
    let generation_options: GenOptions<AExpr> = vec![
        (
            0.4,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::Number(rng.random_range(-100..=100))
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.9
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::binary(
                    gen_aexpr_simple(cx, rng),
                    gen_aop(&mut cx.compiler_context, rng),
                    gen_aexpr_simple(cx, rng),
                )
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.4
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::Minus(Box::new(gen_aexpr_simple(cx, rng)))
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_bexpr_nondet<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        (2.0, Box::new(|_, _| BExpr::Bool(true))),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, _| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Not(Box::new(BExpr::Bool(false)))
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Not(Box::new(gen_bexpr_stuck(cx, rng)))
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.5
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_nondet(cx, rng)),
                    LogicOp::Or,
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                    LogicOp::Or,
                    Box::new(gen_bexpr_nondet(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_nondet(cx, rng)),
                    LogicOp::Lor,
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                    LogicOp::Lor,
                    Box::new(gen_bexpr_nondet(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_nondet(cx, rng)),
                    LogicOp::Or,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_nondet(cx, rng)),
                    LogicOp::Lor,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                gen_bexpr_nondet_rel(cx, rng)
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_bexpr_nondet_rel<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let a_axper = gen_aexpr_simple(cx, rng);
                let b_axper = gen_aexpr_simple(cx, rng);

                let (a_var_min, _, a_ex) = aexpr_resolve(a_axper.clone(), cx);
                let (b_var_min, _, b_ex) = aexpr_resolve(b_axper.clone(), cx);

                BExpr::Rel(
                    if a_ex {
                        AExpr::Number(a_var_min)
                    } else {
                        a_axper
                    },
                    RelOp::Eq,
                    if b_ex {
                        AExpr::Number(b_var_min)
                    } else {
                        AExpr::Binary(
                            Box::new(b_axper),
                            AOp::Plus,
                            Box::new(AExpr::Number(a_var_min - b_var_min)),
                        )
                    },
                )
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let aexpr = gen_aexpr_simple(cx, rng);

                BExpr::Rel(aexpr.clone(), RelOp::Eq, aexpr)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                BExpr::Rel(
                    gen_aexpr_simple(cx, rng),
                    RelOp::Ne,
                    gen_aexpr_simple(cx, rng),
                )
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let (small, large, _) =
                    sort_aexpr(gen_aexpr_simple(cx, rng), gen_aexpr_simple(cx, rng), cx);

                BExpr::Rel(large, RelOp::Ge, small)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let (small, large, _) =
                    sort_aexpr(gen_aexpr_simple(cx, rng), gen_aexpr_simple(cx, rng), cx);

                BExpr::Rel(large, RelOp::Gt, small)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let (small, large, _) =
                    sort_aexpr(gen_aexpr_simple(cx, rng), gen_aexpr_simple(cx, rng), cx);

                BExpr::Rel(small, RelOp::Le, large)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                cx.compiler_context.recursion_limit = cx.compiler_context.negation_limit;

                let (small, large, _) =
                    sort_aexpr(gen_aexpr_simple(cx, rng), gen_aexpr_simple(cx, rng), cx);

                BExpr::Rel(small, RelOp::Lt, large)
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_bexpr_stuck<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        (2.0, Box::new(|_, _| BExpr::Bool(false))),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, _| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Not(Box::new(BExpr::Bool(true)))
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Not(Box::new(BExpr::Not(Box::new(gen_bexpr_stuck(cx, rng)))))
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_stuck(cx, rng)),
                    LogicOp::And,
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                    LogicOp::And,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_stuck(cx, rng)),
                    LogicOp::Land,
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr(&mut cx.compiler_context, rng)),
                    LogicOp::Land,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_stuck(cx, rng)),
                    LogicOp::Or,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                0.25
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Logic(
                    Box::new(gen_bexpr_stuck(cx, rng)),
                    LogicOp::Lor,
                    Box::new(gen_bexpr_stuck(cx, rng)),
                )
            }),
        ),
        (
            if cx.compiler_context.negation_limit == 0 {
                0.0
            } else {
                1.0
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                gen_bexpr_stuck_rel(cx, rng)
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_bexpr_stuck_rel<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                BExpr::Rel(
                    gen_aexpr(&mut cx.compiler_context, rng),
                    RelOp::Eq,
                    gen_aexpr(&mut cx.compiler_context, rng),
                )
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();

                let aexpr = gen_aexpr(&mut cx.compiler_context, rng);

                BExpr::Rel(aexpr.clone(), RelOp::Ne, aexpr)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();

                let (small, large, _) = sort_aexpr(
                    gen_aexpr(&mut cx.compiler_context, rng),
                    gen_aexpr(&mut cx.compiler_context, rng),
                    cx,
                );

                BExpr::Rel(small, RelOp::Ge, large)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();

                let (small, large, _) = sort_aexpr(
                    gen_aexpr(&mut cx.compiler_context, rng),
                    gen_aexpr(&mut cx.compiler_context, rng),
                    cx,
                );

                BExpr::Rel(small, RelOp::Gt, large)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();

                let (small, large, _) = sort_aexpr(
                    gen_aexpr(&mut cx.compiler_context, rng),
                    gen_aexpr(&mut cx.compiler_context, rng),
                    cx,
                );

                BExpr::Rel(large, RelOp::Le, small)
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.negation_limit = cx
                    .compiler_context
                    .negation_limit
                    .checked_sub(1)
                    .unwrap_or_default();

                let (small, large, _) = sort_aexpr(
                    gen_aexpr(&mut cx.compiler_context, rng),
                    gen_aexpr(&mut cx.compiler_context, rng),
                    cx,
                );

                BExpr::Rel(large, RelOp::Lt, small)
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn sort_aexpr(a: AExpr, b: AExpr, cx: &InterpreterContext) -> (AExpr, AExpr, bool) {
    let (a_var_min, a_var_max, a_ex) = aexpr_resolve(a.clone(), cx);
    let (b_var_min, b_var_max, b_ex) = aexpr_resolve(b.clone(), cx);

    let ex = a_ex || b_ex;

    if a_var_min.min(b_var_min) <= a_var_max.max(b_var_max) {
        // (smallest, largest)
        (a, b, ex)
    } else {
        (b, a, ex)
    }
}

// take an aritmetic expression and returns its bounds (min, max, exeception)
pub fn aexpr_resolve(a: AExpr, cx: &InterpreterContext) -> (i32, i32, bool) {
    match a {
        AExpr::Number(n) => (n, n, false),
        AExpr::Reference(target) => match target {
            Target::Variable(var) => {
                let mem_var = cx.memory.variables[&var];
                (mem_var, mem_var, false)
            }
            Target::Array(arr, axper) => {
                let mem_arr: Vec<i32> = cx.memory.arrays[&arr].clone();
                let (var_min, var_max, a_ex) = aexpr_resolve(axper.simplify(), cx);

                let mut ex = a_ex;

                if var_min < 0 || (mem_arr.len() as i32) <= var_max {
                    ex = true;
                } else if var_min == var_max {
                    let var = mem_arr[var_min as usize];
                    return (var, var, ex);
                }

                (
                    { mem_arr.iter().min().unwrap_or(&0).clone() },
                    { mem_arr.iter().max().unwrap_or(&0).clone() },
                    ex,
                )
            }
        },
        AExpr::Binary(l_aexpr, aop, r_aexpr) => {
            let (l_var_min, l_var_max, l_ex) = aexpr_resolve(l_aexpr.simplify(), cx);
            let (r_var_min, r_var_max, r_ex) = aexpr_resolve(r_aexpr.simplify(), cx);

            let var_min = || l_var_min.min(r_var_min);
            let var_max = || l_var_max.max(r_var_max);
            let mut ex = l_ex || r_ex;

            match aop {
                AOp::Plus => (
                    l_var_min.saturating_add(r_var_min),
                    l_var_max.saturating_add(r_var_max),
                    ex,
                ),
                AOp::Minus => (
                    l_var_min.saturating_sub(r_var_min),
                    l_var_max.saturating_sub(r_var_max),
                    ex,
                ),
                AOp::Times => {
                    // test all combinations
                    let liri = l_var_min.saturating_mul(r_var_min);
                    let lira = l_var_min.saturating_mul(r_var_max);
                    let lari = l_var_max.saturating_mul(r_var_min);
                    let lara = l_var_max.saturating_mul(r_var_max);

                    (
                        // min of all values
                        liri.min(lira).min(lari).min(lara),
                        // max of all values
                        liri.max(lira).max(lari).max(lara),
                        ex,
                    )
                }
                AOp::Divide => {
                    // check if a division by 0 is possible

                    if r_var_min <= 0 && r_var_max >= 0 {
                        ex = true;
                    }

                    if !(l_var_min == 0 || r_var_max == 0) {
                        let div = |l: i32, r: i32| -> i32 {
                            // should never panic
                            l.checked_div(r).unwrap()
                        };

                        let liri = div(l_var_min, r_var_min);
                        let lira = div(l_var_min, r_var_max);
                        let lari = div(l_var_max, r_var_min);
                        let lara = div(l_var_max, r_var_max);

                        (
                            // min of all values
                            liri.min(lira).min(lari).min(lara),
                            // max of all values
                            liri.max(lira).max(lari).max(lara),
                            ex,
                        )
                    } else {
                        let mut div_min = |l: i32, r: i32| -> i32 {
                            l.checked_div(r).unwrap_or({
                                ex = true;
                                var_min()
                            })
                        };

                        let liri_min = div_min(l_var_min, r_var_min);
                        let lira_min = div_min(l_var_min, r_var_max);
                        let lari_min = div_min(l_var_max, r_var_min);
                        let lara_min = div_min(l_var_max, r_var_max);

                        let mut div_max = |l: i32, r: i32| -> i32 {
                            l.checked_div(r).unwrap_or({
                                ex = true;
                                var_max()
                            })
                        };
                        let liri_max = div_max(l_var_min, r_var_min);
                        let lira_max = div_max(l_var_min, r_var_max);
                        let lari_max = div_max(l_var_max, r_var_min);
                        let lara_max = div_max(l_var_max, r_var_max);

                        (
                            // min of all values
                            liri_min.min(lira_min).min(lari_min).min(lara_min),
                            // max of all values
                            liri_max.max(lira_max).max(lari_max).max(lara_max),
                            ex,
                        )
                    }
                }
                AOp::Pow => {
                    if r_var_min < 0 || r_var_max < 0 {
                        (var_min(), var_max(), true)
                    } else if l_var_min >= 0 || l_var_max >= 0 {
                        let to_u32 = |n: i32| -> u32 {
                            match n.try_into() {
                                Ok(var) => var,
                                Err(_) => match (-n).try_into() {
                                    Ok(var) => var,
                                    Err(_) => 1,
                                },
                            }
                        };

                        let pow_min = l_var_min.checked_pow(to_u32(r_var_min)).unwrap_or({
                            ex = true;
                            var_min()
                        });

                        let pow_max = l_var_max.checked_pow(to_u32(r_var_max)).unwrap_or({
                            ex = true;
                            var_max()
                        });

                        (pow_min, pow_max, ex)
                    } else {
                        (var_min(), var_max(), true)
                    }
                }
            }
        }
        AExpr::Minus(aexpr) => {
            let (var_min, var_max, ex) = aexpr_resolve(aexpr.simplify(), cx);
            (var_min, var_max, ex)
        }
    }
}

pub fn gen_multiple_guards(
    cx: &mut InterpreterContext,
    rng: &mut ErasedRng,
    gnopt: &GenOptionsNested<Commands>,
) -> Vec<Guard> {
    let n = rng.random_range(2..=(cx.compiler_context.fuel.max(2) / 2).max(2));

    let guards: Vec<Guard> = (0..n)
        .map(|_| Guard(gen_bexpr_nondet(cx, rng), gnopt.generate(cx, rng)))
        .collect();

    guards
}

pub fn gen_undefined_guards(
    cx: &mut InterpreterContext,
    rng: &mut ErasedRng,
    gnopt: &GenOptionsNested<Commands>,
) -> Vec<Guard> {
    let n = rng.random_range(1..(cx.compiler_context.fuel.max(2) / 2).max(2));

    let guards: Vec<Guard> = (0..n)
        .map(|_| Guard(gen_bexpr_undefined(cx, rng), gnopt.generate(cx, rng)))
        .collect();

    guards
}

pub fn gen_bexpr_undefined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> BExpr {
    let generation_options: GenOptions<BExpr> = vec![
        // BExpr::
        // =
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Eq, AExpr::Number(0))
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Ne, AExpr::Number(0))
            }),
        ),
        // >
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Gt, AExpr::Number(0))
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Lt, AExpr::Number(0))
            }),
        ),
        // >=
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Ge, AExpr::Number(0))
            }),
        ),
        (
            1.0,
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                BExpr::Rel(gen_aexpr_op_undefined(cx, rng), RelOp::Le, AExpr::Number(0))
            }),
        ),
        // ^
        // &&
        // not
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: BExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_aexpr_op_undefined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> AExpr {
    let generation_options: GenOptions<AExpr> = vec![(
        0.20,
        Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
            cx.compiler_context.recursion_limit = cx
                .compiler_context
                .recursion_limit
                .checked_sub(1)
                .unwrap_or_default();
            AExpr::binary(
                gen_aexpr(&mut cx.compiler_context, rng),
                AOp::Divide,
                AExpr::Number(0),
            )
        }),
    )];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: AExpr = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

pub fn gen_aexpr_undefined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> AExpr {
    let generation_options: GenOptions<AExpr> = vec![
        (
            0.4,
            Box::new(|_cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                AExpr::Number(rng.random_range(-100..=-1))
            }),
        ),
        (
            if cx.compiler_context.names.is_empty() && cx.compiler_context.array_names.is_empty() {
                0.0
            } else {
                0.8
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                AExpr::Reference(gen_reference_undefined(cx, rng))
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.9
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::binary(
                    gen_aexpr_undefined(cx, rng),
                    gen_aop(&mut cx.compiler_context, rng),
                    gen_aexpr_undefined(cx, rng),
                )
            }),
        ),
        (
            if cx.compiler_context.recursion_limit == 0 || cx.compiler_context.fuel == 0 {
                0.0
            } else {
                0.4
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                cx.compiler_context.recursion_limit = cx
                    .compiler_context
                    .recursion_limit
                    .checked_sub(1)
                    .unwrap_or_default();
                AExpr::Minus(Box::new(gen_aexpr_undefined(cx, rng)))
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}

fn gen_reference_undefined<R: Rng>(cx: &mut InterpreterContext, rng: &mut R) -> Target<Box<AExpr>> {
    let generation_options: GenOptions<Target<Box<AExpr>>> = vec![
        (
            if cx.compiler_context.names.is_empty() {
                0.0
            } else {
                0.7
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                Target::Variable(Variable(
                    cx.compiler_context.names.choose(rng).cloned().unwrap(),
                ))
            }),
        ),
        (
            if cx.compiler_context.no_arrays {
                0.0
            } else {
                0.3
            },
            Box::new(|cx: &mut InterpreterContext, rng: &mut ErasedRng| {
                let name = cx
                    .compiler_context
                    .array_names
                    .choose(rng)
                    .cloned()
                    .unwrap_or_else(|| "A".into());

                let mut aexpr = gen_aexpr_defined(cx, rng);
                let (small, large, _) = aexpr_resolve(aexpr.clone(), cx);

                if large < 0 {
                    aexpr = AExpr::Minus(Box::new(aexpr));
                }
                if small < 0 {
                    if rng.random_bool(0.5) {
                        aexpr = AExpr::Binary(
                            Box::new(aexpr),
                            AOp::Minus,
                            Box::new(AExpr::Number(small)),
                        );
                    } else {
                        aexpr = AExpr::Binary(
                            Box::new(AExpr::Minus(Box::new(AExpr::Number(small)))),
                            AOp::Plus,
                            Box::new(aexpr),
                        );
                    }
                }

                // brute force fix as references are not carried over
                if large > 5 {
                    aexpr = AExpr::Binary(
                        Box::new(aexpr),
                        AOp::Minus,
                        Box::new(AExpr::Number((large + 1) / 2)),
                    );
                }

                Target::Array(Array(name), Box::new(aexpr))
            }),
        ),
    ];

    let mut erng = SmallRng::seed_from_u64(rng.random());

    let choice: Target<Box<AExpr>> = generation_options
        .choose_weighted(&mut erng, |item| item.0)
        .unwrap()
        .1(cx, &mut erng);

    choice
}
