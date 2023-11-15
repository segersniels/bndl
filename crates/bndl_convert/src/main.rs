use std::{env, process};

use bndl_convert::{fetch_tsconfig, SerializableConfig};
use clap::{ArgAction, Command};

fn cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .about("Convert a tsconfig.json to an SWC compatible config")
        .version(env!("CARGO_PKG_VERSION"))
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .arg(
            clap::Arg::new("minify")
                .short('m')
                .long("minify")
                .help("Minify the output bundle")
                .action(ArgAction::SetTrue),
        )
}

fn main() {
    let matches = cli().get_matches();
    let minify_output = matches.get_flag("minify");
    let filename = match matches.subcommand() {
        Some((query, _)) => query,
        _ => "tsconfig.json",
    };

    match fetch_tsconfig(filename) {
        Ok(tsconfig) => {
            let config = bndl_convert::convert(&tsconfig, Some(minify_output), None);
            println!(
                "{}",
                serde_json::to_string_pretty(&SerializableConfig::from(&config)).unwrap()
            );
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
