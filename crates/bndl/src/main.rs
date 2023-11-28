use clap::{ArgAction, Command};
use human_panic::setup_panic;
use std::path::PathBuf;

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
        .arg(
            clap::Arg::new("watch")
                .short('w')
                .long("watch")
                .help("Experimental: watch the input files for changes and recompile when they change")
                .action(ArgAction::SetTrue),
        )
}

fn main() {
    env_logger::init();
    setup_panic!();

    let matches = cli().get_matches();
    let default_config = String::from("tsconfig.json");
    let mut config_path = matches
        .get_one::<String>("project")
        .unwrap_or(&default_config);

    if *config_path == "." {
        config_path = &default_config;
    }

    let input_path = match matches.subcommand() {
        Some((query, _)) => PathBuf::from(query),
        _ => PathBuf::from("."),
    };

    let default_out_dir = String::from("dist");
    let out_dir = matches
        .get_one::<String>("outDir")
        .unwrap_or(&default_out_dir);

    if matches.get_flag("watch") {
        if let Err(err) = utils::compile::watch(TranspileOptions {
            input_path,
            out_dir: PathBuf::from(out_dir),
            config_path: PathBuf::from(config_path),
            minify_output: matches.get_flag("minify"),
            bundle: !matches.get_flag("no-bundle"),
            clean: false,
        }) {
            eprintln!("{err}");
        }

        return;
    }

    if let Err(err) = utils::compile::transpile(TranspileOptions {
        input_path,
        out_dir: PathBuf::from(out_dir),
        config_path: PathBuf::from(config_path),
        minify_output: matches.get_flag("minify"),
        clean: matches.get_flag("clean"),
        bundle: !matches.get_flag("no-bundle"),
    }) {
        eprintln!("{err}");
    }
}
