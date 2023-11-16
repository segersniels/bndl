use std::path::Path;

use bndl_convert::fetch_tsconfig;
use clap::{ArgAction, Command};
use human_panic::setup_panic;

mod utils;

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .about("Experimental Rust based bundler for TypeScript")
        .version(env!("CARGO_PKG_VERSION"))
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .arg(
            clap::Arg::new("project")
                .short('p')
                .long("project")
                .help("The path to the project config file")
                .action(ArgAction::Set),
        )
        .arg(
            clap::Arg::new("outDir")
                .long("outDir")
                .help("Specify an output folder for all emitted files.")
                .action(ArgAction::Set),
        )
        .arg(
            clap::Arg::new("clean")
                .long("clean")
                .help("Clean the output folder if it exists before bundling")
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("no-bundle")
                .long("no-bundle")
                .help("Disable automatic bundling of internal  monorepo dependencies")
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("minify")
                .short('m')
                .long("minify")
                .help("Minify the output bundle")
                .action(ArgAction::SetTrue),
        )
}

fn main() {
    env_logger::init();
    setup_panic!();

    let matches = cli().get_matches();
    let default_config = String::from("tsconfig.json");
    let config_path = matches
        .get_one::<String>("project")
        .unwrap_or(&default_config);

    let minify_output = matches.get_flag("minify");
    let input_path = match matches.subcommand() {
        Some((query, _)) => Path::new(query),
        _ => Path::new("."),
    };

    let result = fetch_tsconfig(config_path);
    match result {
        Ok(ts_config) => {
            let ts_config_out_dir =
                if let Some(compiler_options) = ts_config.clone().compilerOptions {
                    compiler_options.outDir.unwrap_or_default()
                } else {
                    String::from("dist")
                };

            let out_path = Path::new(
                matches
                    .get_one::<String>("outDir")
                    .unwrap_or(&ts_config_out_dir),
            );

            // Clean the output directory if the flag is set
            if matches.get_flag("clean") {
                utils::compile::clean_out_dir(out_path);
            }

            // Transpile the code to javascript
            utils::compile::transpile(input_path, out_path, &ts_config, config_path, minify_output);

            // Bundle the monorepo dependencies if the flag is set
            if !matches.get_flag("no-bundle") {
                utils::bundle::bundle(out_path);
            }
        }
        Err(e) => {
            eprintln!("{}", e)
        }
    }
}
