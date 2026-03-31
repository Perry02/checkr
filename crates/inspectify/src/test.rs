use std::{collections::HashSet, fs, str::FromStr};

use crate::endpoints::InspectifyJobMeta;
use ce_core::Generate;
use ce_shell::Analysis;
use driver::JobKind;
use gcl::pg::Determinism;
use rand::{SeedableRng, seq::IndexedRandom};
use roxmltree::Document;

fn covered_lines(xml: &str) -> HashSet<(String, u32)> {
    let doc = Document::parse(xml).unwrap();
    let mut covered = HashSet::new();

    for class in doc.descendants().filter(|n| n.has_tag_name("class")) {
        let filename = class.attribute("filename").unwrap_or("").to_string();
        for line in class
            .children()
            .filter(|n| n.has_tag_name("lines"))
            .flat_map(|ls| ls.children().filter(|n| n.has_tag_name("line")))
        {
            let hits: u64 = line.attribute("hits").unwrap_or("0").parse().unwrap_or(0);
            if hits > 0 {
                let number: u32 = line.attribute("number").unwrap_or("0").parse().unwrap_or(0);
                covered.insert((filename.clone(), number));
            }
        }
    }

    covered
}

/// Runs dotnet-coverage for `test_amount` seeds using the provided arg generator.
/// Returns (unique_lines_hit, total_instrumentable_lines).
/// Unique lines = union of all lines hit across all seeds.
async fn coverage_test(
    hub: &driver::Hub<InspectifyJobMeta>,
    cwd: &std::path::PathBuf,
    driver: &driver::Driver<InspectifyJobMeta>,
    label: &str,
    test_amount: usize,
    mut get_args: impl FnMut(usize) -> (String, String),
) -> (usize, usize) {
    let run_exe = cwd.join(driver.config().run().split(' ').next().unwrap());
    let run_exe_str = run_exe.to_string_lossy().into_owned();

    let mut union_covered: HashSet<(String, u32)> = HashSet::new();
    let mut total_possible = 0usize;

    for index in 1..=test_amount {
        print!("  [{label}] seed {index}...");

        let (program, args) = get_args(index);
        // DEBUG: show the program being tested
        println!(
            "    program: {program}, args (first 120 chars): {}",
            &args[..args.len().min(120)]
        );

        let job = hub.exec_command(
            JobKind::Compilation,
            cwd.clone(),
            InspectifyJobMeta::default(),
            "dotnet-coverage",
            [
                "collect",
                "--output-format",
                "cobertura",
                "--output",
                "coverage.xml",
                run_exe_str.as_str(),
                program.as_str(),
                args.as_str(),
            ],
        );

        job.wait().await;
        // debugging lines
        let out = job.stdout();
        let err = job.stderr();
        if !out.trim().is_empty() {
            println!("    stdout: {}", &out[..out.len().min(200)]);
        }
        if !err.trim().is_empty() {
            println!("    stderr: {}", &err[..err.len().min(200)]);
        }

        let xml_path = cwd.join("coverage.xml");
        let xml = fs::read_to_string(&xml_path).expect("coverage.xml not found");

        if total_possible == 0 {
            let doc = Document::parse(&xml).unwrap();
            for _ in doc.descendants().filter(|n| n.has_tag_name("line")) {
                total_possible += 1;
            }
        }

        let this_run = covered_lines(&xml);
        let hit_this_run = this_run.len();
        let new_this_seed = this_run.difference(&union_covered).count();
        union_covered.extend(this_run.clone());

        // debugging lines
        let mut files: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for (f, _) in &this_run {
            files.insert(f.as_str());
        }
        println!("    covered files: {files:?}");

        println!(
            "    hit {hit_this_run} unique lines this run (+{new_this_seed} new, cumulative: {})",
            union_covered.len()
        );
    }

    (union_covered.len(), total_possible)
}

/// Serialize only the fields F#'s Io.Compiler.Input expects: { commands, determinism }.
fn to_fsharp_compiler_json(commands: &gcl::ast::Commands, determinism: Determinism) -> String {
    let commands_str = commands.to_string();
    let det_str = match determinism {
        Determinism::Deterministic => "Deterministic",
        Determinism::NonDeterministic => "NonDeterministic",
    };
    format!(
        r#"{{"commands":{},"determinism":"{}"}}"#,
        serde_json::to_string(&commands_str).unwrap(),
        det_str
    )
    .replace('"', "\\\"")
}

/// Generate a Compiler input using the OLD gcl_gen
fn compiler_input_old_gen(seed: usize) -> (String, String) {
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed as u64);
    let commands = gcl::ast::Commands::gn(&mut Default::default(), &mut rng);
    let determinism = *[Determinism::Deterministic, Determinism::NonDeterministic]
        .choose(&mut rng)
        .unwrap();
    (
        "Compiler".to_string(),
        to_fsharp_compiler_json(&commands, determinism),
    )
}

/// Generate a Compiler input using the NEW gcl_compiler_gen
fn compiler_input_new_gen(seed: usize) -> (String, String) {
    use ce_core::gn::compiler_gen::{CompilerContext, gen_commands};
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed as u64);
    let commands = gen_commands(
        &mut CompilerContext {
            fuel: 30,
            ..Default::default()
        },
        &mut rng,
    );
    let determinism = *[Determinism::Deterministic, Determinism::NonDeterministic]
        .choose(&mut rng)
        .unwrap();
    (
        "Compiler".to_string(),
        to_fsharp_compiler_json(&commands, determinism),
    )
}

#[tokio::test]
async fn test_thingy() {
    // step 1. install dotnet-coverage: "dotnet tool install -g dotnet-coverage"
    // step 2. run "dotnet publish -c Release --self-contained --output bin/run" in the F# project root
    // step 3. set path_to_fsharp to the F# project root
    // step 4. set test_amount to the number of seeds per generator
    // step 5. run this test

    let hub: driver::Hub<InspectifyJobMeta> = driver::Hub::new().expect("");
    let path_to_fsharp = "D:/checkr/student_implementation/Group-03-Sorbet-Seagulls/code";
    let cwd = dunce::canonicalize(path_to_fsharp).expect("msg");
    let driver =
        driver::Driver::new_from_path(hub.clone(), ".", path_to_fsharp.to_owned() + "/run.toml")
            .expect("");

    driver.ensure_compile(InspectifyJobMeta::default());

    let test_amount = 5000;

    // Compiler with OLD gcl_gen
    println!("\n=== Compiler (old gcl_gen)");
    let (old_unique, old_total) = coverage_test(
        &hub,
        &cwd,
        &driver,
        "old gcl_gen",
        test_amount,
        compiler_input_old_gen,
    )
    .await;
    let old_pct = old_unique as f64 / old_total as f64 * 100.0;

    // Compiler with NEW gcl_compiler_gen
    println!("\n=== Compiler (new gcl_compiler_gen)");
    let (new_unique, new_total) = coverage_test(
        &hub,
        &cwd,
        &driver,
        "new gcl_compiler_gen",
        test_amount,
        compiler_input_new_gen,
    )
    .await;
    let new_pct = new_unique as f64 / new_total as f64 * 100.0;

    // Summary
    println!("\n=== Coverage comparison ({test_amount} seeds each) ===");
    println!("Old gcl_gen         — unique lines: {old_unique}/{old_total} = {old_pct:.2}%");
    println!("New gcl_compiler_gen — unique lines: {new_unique}/{new_total} = {new_pct:.2}%");
    println!(
        "Improvement: {:+.2}% ({:+} unique lines)",
        new_pct - old_pct,
        new_unique as i64 - old_unique as i64,
    );
}
