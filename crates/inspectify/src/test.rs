use std::{fs, str::FromStr};

use crate::endpoints::InspectifyJobMeta;
use ce_shell::{Analysis, Input};
use color_eyre::eyre::Context;
use driver::JobKind;
use itertools::Itertools as _;
use roxmltree::Document;
use tapi::{Tapi, endpoints::RouterExt};

#[tokio::test]
async fn test_thingy() {
    let hub: driver::Hub<InspectifyJobMeta> = driver::Hub::new().expect("");

    let path_to_fsharp = "../.././starters/fsharp-starter";
    let cwd = dunce::canonicalize(path_to_fsharp).expect("msg");

    let driver =
        driver::Driver::new_from_path(hub.clone(), ".", "../.././starters/fsharp-starter/run.toml")
            .expect("");

    driver.ensure_compile(InspectifyJobMeta::default());

    // for option in Analysis::options() {
    //     println!("analysis: {option}");
    // }

    let analysis = Analysis::from_str("Interpreter").expect("failure");

    // dotnet tool install -g dotnet-coverage

    let test_amount = 5;
    let mut total_lines = 0;

    for index in (1..test_amount + 1).rev() {
        print!("running seed {index}...");

        let input = analysis.gen_input_seeded(Some(index));

        // let job = driver.exec_job(&input, InspectifyJobMeta::default());

        // job.wait().await;

        // let output = input.analysis().output_from_str(&job.stdout());

        // if output.is_err() {
        //     break;
        // }

        // let success = input.validate_output(&output.expect("")).expect("msg");

        let program = input.analysis().code().to_string();
        let args = input.to_string();

        // println!("cwd: {}", cwd.clone().to_str().expect("msg"));
        // println!(
        //     "driver: {}",
        //     driver
        //         .config()
        //         .run()
        //         .split(' ')
        //         .map(|s| s.to_string())
        //         .collect_vec()[0]
        //         .as_str()
        // );
        // println!("program: {}", program);
        // println!("args: {}", args);

        let job2 = hub.exec_command(
            JobKind::Analysis(analysis.gen_input_seeded(Some(0)).clone()),
            cwd.clone(),
            InspectifyJobMeta::default(),
            "dotnet-coverage",
            [
                "collect",
                "--output-format",
                "cobertura",
                "--output",
                "coverage.xml",
                driver
                    .config()
                    .run()
                    .split(' ')
                    .map(|s| s.to_string())
                    .collect_vec()[0]
                    .as_str(),
                program.as_str(),
                args.as_str(),
            ],
        );

        job2.wait().await;

        // read xml

        let path_to_xml = String::from(path_to_fsharp.to_owned() + "/coverage.xml");

        let xml_path = std::path::Path::new(&path_to_xml);

        let xml = fs::read_to_string(xml_path).expect("msg");

        let doc = Document::parse(&xml).unwrap();
        let mut count = 0;

        // Cobertura format: <line number="10" hits="1" />
        for node in doc.descendants().filter(|n| n.has_tag_name("line")) {
            let hits: usize = node.attribute("hits").unwrap_or("0").parse().unwrap_or(0);
            if hits > 0 {
                count = count + 1;
            }
        }
        total_lines += count;
        println!("finished. Hit {count} lines");
        //println!("{}", &job2.stdout());
    }

    println!(
        "Finished {test_amount} runs and hit a total of {total_lines} lines with an average of {} lines",
        total_lines / test_amount
    );

    assert!(true)
}
