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
            clap::Arg::new("legacy-dts")
                .short('d')
                .long("legacy-dts")
                .help("Generate .d.ts files from TypeScript and JavaScript files in your project (bypasses SWC and uses `tsc`).")
                .action(ArgAction::SetTrue),
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
                .action(ArgAction::SetTrue)
        )
        .arg(
            clap::Arg::new("bundle")
                .short('b')
                .long("bundle")
                .help("Attempt barebones bundling of the project")
                .action(ArgAction::SetTrue)
        )
}

fn main() {
    setup_panic!();

    let matches = cli().get_matches();
    let filename = match matches.subcommand() {
        Some((query, _)) => query,
        _ => ".",
    };

    let default_config = String::from("tsconfig.json");
    let config_path = matches
        .get_one::<String>("project")
        .unwrap_or(&default_config);

    let default_out_dir = String::from("dist");
    let out_dir = matches
        .get_one::<String>("outDir")
        .unwrap_or(&default_out_dir);

    // Clean the output directory if the flag is set
    if matches.get_flag("clean") {
        utils::compile::clean_out_dir(out_dir);
    }

    // Transpile the code to javascript
    let fallback_legacy_dts = matches.get_flag("legacy-dts");
    utils::compile::transpile(filename, out_dir, config_path, fallback_legacy_dts);

    // Rely on `tsc` to provide .d.ts files since SWC's implementation is a bit weird
    if fallback_legacy_dts {
        utils::compile::create_tsc_dts(config_path, out_dir);
    }

    // Bundle the monorepo dependencies if the flag is set
    if matches.get_flag("bundle") {
        utils::bundle::bundle(out_dir);
    }
}
