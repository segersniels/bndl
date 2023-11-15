use std::{env, process};

use bndl_convert::{fetch_tsconfig, SerializableConfig};
use clap::{ArgAction, Command};
use serde_json::Value;
use swc::config::Config;

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

fn remove_unwanted_values(value: &mut Value) -> Value {
    match value {
        Value::Object(map) => {
            let keys_to_remove: Vec<String> = map
                .iter()
                .filter(|(_, v)| v.is_null() || (v.is_string() && v.to_string().is_empty()))
                .map(|(k, _)| k.clone())
                .collect();

            // Remove keys with `null` values
            for key in keys_to_remove {
                map.remove(&key);
            }

            // Now, iterate over the map and recursively remove on each value
            let empty_keys: Vec<String> = map
                .iter_mut()
                .filter_map(|(k, v)| {
                    remove_unwanted_values(v); // Recursively clean each value
                    if v.is_object() && v.as_object().map(|m| m.is_empty()).unwrap_or(false) {
                        Some(k.clone()) // Collect keys of now empty objects for removal
                    } else {
                        None
                    }
                })
                .collect();

            // Remove keys that have become empty objects after the recursive cleaning
            for key in empty_keys {
                map.remove(&key);
            }
        }
        Value::Array(vec) => {
            // Clean each element in the array
            for v in vec.iter_mut() {
                remove_unwanted_values(v);
            }
            // Remove `null` elements and empty objects from the array
            vec.retain(|v| {
                !v.is_null()
                    && !(v.is_object() && v.as_object().map(|m| m.is_empty()).unwrap_or(false))
            });
        }
        // For `null` or other types, do nothing
        _ => (),
    }

    return value.to_owned();
}

/// Remove `null` values and empty objects from the config before logging
fn parse_config_before_logging(config: &Config) -> Value {
    let mut value = serde_json::to_value(SerializableConfig::from(config)).unwrap();

    remove_unwanted_values(&mut value)
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
            let converted_config = bndl_convert::convert(&tsconfig, Some(minify_output), None);
            let cleaned_config = parse_config_before_logging(&converted_config);

            println!("{}", serde_json::to_string_pretty(&cleaned_config).unwrap());
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}
