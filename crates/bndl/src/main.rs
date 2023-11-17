use std::path::PathBuf;

use clap::{ArgAction, Command};
use human_panic::setup_panic;

use crate::utils::compile::TranspileOptions;

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

    let input_path = match matches.subcommand() {
        Some((query, _)) => PathBuf::from(query),
        _ => PathBuf::from("."),
    };

    // Transpile the code to javascript
    if let Err(err) = utils::compile::transpile(TranspileOptions {
        filename: input_path,
        out_dir: matches.get_one::<String>("outDir").cloned(),
        config_path: PathBuf::from(config_path),
        minify_output: matches.get_flag("minify"),
        clean: matches.get_flag("clean"),
        bundle: !matches.get_flag("no-bundle"),
    }) {
        eprintln!("{err}");
    }
}
