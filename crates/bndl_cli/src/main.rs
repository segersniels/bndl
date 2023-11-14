use clap::{ArgAction, ArgMatches, Command};
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
            clap::Arg::new("bundle")
                .short('b')
                .long("bundle")
                .help("Attempt barebones bundling of the project")
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

/// Determines the variables to use for the bundling process based
/// on the command line arguments provided
fn determine_variables(matches: &ArgMatches) -> (String, String) {
    let default_config = String::from("tsconfig.json");
    let config_path = matches
        .get_one::<String>("project")
        .unwrap_or(&default_config);

    let default_out_dir = String::from("dist");
    let out_dir = matches
        .get_one::<String>("outDir")
        .unwrap_or(&default_out_dir);

    (config_path.to_owned(), out_dir.to_owned())
}

fn main() {
    env_logger::init();
    setup_panic!();

    let matches = cli().get_matches();
    let (config_path, out_dir) = determine_variables(&matches);

    // Clean the output directory if the flag is set
    if matches.get_flag("clean") {
        utils::compile::clean_out_dir(&out_dir);
    }

    // Transpile the code to javascript
    let minify_output = matches.get_flag("minify");
    let filename = match matches.subcommand() {
        Some((query, _)) => query,
        _ => ".",
    };

    utils::compile::transpile(filename, &out_dir, &config_path, minify_output);

    // Bundle the monorepo dependencies if the flag is set
    if matches.get_flag("bundle") {
        utils::bundle::bundle(&out_dir);
    }
}
