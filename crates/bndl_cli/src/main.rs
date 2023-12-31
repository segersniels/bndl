use clap::{ArgAction, Command};
use human_panic::setup_panic;
use std::{path::PathBuf, process};

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
            clap::Arg::new("only-bundle")
                .long("only-bundle")
                .help("Skips compilation and only bundles the input files, assuming they are already compiled beforehand")
                .action(ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("no-bundle")
                .long("no-bundle")
                .help("Disable automatic bundling of internal monorepo dependencies")
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

    // Determine the config path
    let default_config = String::from("tsconfig.json");
    let mut config_path = matches
        .get_one::<String>("project")
        .unwrap_or(&default_config);

    // In `tsc` you can specify the config as `.` to use the default config
    if *config_path == "." {
        config_path = &default_config;
    }

    // Determine the input path
    let input_path = match matches.subcommand() {
        Some((query, _)) => PathBuf::from(query),
        _ => PathBuf::from("."),
    };

    // Fetch the tsconfig.json
    let tsconfig = match bndl_convert::fetch_tsconfig(&PathBuf::from(config_path)) {
        Ok(tsconfig) => tsconfig,
        Err(err) => {
            eprintln!("{err}");
            process::exit(1);
        }
    };

    // Determine the output path (give priority to the optional flag)
    let override_out_dir = matches.get_one::<String>("outDir").map(PathBuf::from);
    let out_dir = bndl_convert::determine_out_dir(&tsconfig, override_out_dir);

    // If requested, only bundle the internal dependencies
    if matches.get_flag("only-bundle") {
        if let Err(err) = utils::bundle::bundle(&out_dir) {
            eprintln!("{err}");
        }

        return;
    }

    // If the watch flag is set, watch the input files for changes and recompile when they change
    if matches.get_flag("watch") {
        if let Err(err) = utils::compile::watch(
            TranspileOptions {
                input_path,
                out_dir,
                config_path: PathBuf::from(config_path),
                minify_output: matches.get_flag("minify"),
                bundle: !matches.get_flag("no-bundle"),
                clean: false,
            },
            tsconfig,
        ) {
            eprintln!("{err}");
            process::exit(1)
        }

        return;
    }

    // Otherwise, just transpile the input files
    if let Err(err) = utils::compile::transpile(
        TranspileOptions {
            input_path,
            out_dir,
            config_path: PathBuf::from(config_path),
            minify_output: matches.get_flag("minify"),
            clean: matches.get_flag("clean"),
            bundle: !matches.get_flag("no-bundle"),
        },
        &tsconfig,
    ) {
        eprintln!("{err}");
        process::exit(1)
    }
}
