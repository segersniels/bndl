use bndl_convert::{convert, fetch_tsconfig};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::{fs, process};
use std::{path::Path, sync::Arc};
use swc::{self, config::Options};
use swc_common::{SourceMap, GLOBALS};
use walkdir::WalkDir;

/// Removes the output directory if it exists
pub fn clean_out_dir(out_dir: &str) {
    if Path::new(out_dir).exists() {
        fs::remove_dir_all(out_dir).expect("Failed to remove directory");
    }
}

/// Creates .d.ts files for the project
pub fn create_tsc_dts(project: &str, out_dir: &str) -> std::process::Output {
    let args = vec![
        "tsc",
        "-d",
        "--emitDeclarationOnly",
        "--outDir",
        out_dir,
        "--project",
        project,
    ];

    std::process::Command::new("npx")
        .args(args)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .expect("Failed to execute command")
}

fn write_transpiled_to_file(path: &Path, code: &str, desired_extension: &str) {
    let output_path = Path::new(path).with_extension(desired_extension);

    if let Some(p) = output_path.parent() {
        fs::create_dir_all(p).expect("Failed to create directory");
    };

    fs::write(&output_path, code.as_bytes()).expect("Failed to write to file")
}

fn check_to_ignore_file(filename: &Path, glob_set: &GlobSet) -> bool {
    glob_set.is_match(filename)
}

fn compile_file(
    input_path: &Path,
    out_dir: &Path,
    compiler: &swc::Compiler,
    options: &Options,
    glob_set: &GlobSet,
) {
    // Check if we should ignore the file based on the tsconfig exclude
    // We need to do this because the swc `exclude`` is odd and doesn't work as expected
    if check_to_ignore_file(input_path, glob_set) {
        return;
    }

    let cm = Arc::<SourceMap>::default();
    let output_path = out_dir.join(input_path);
    let output = GLOBALS.set(&Default::default(), || {
        swc::try_with_handler(cm.clone(), Default::default(), |handler| {
            let fm = cm
                .load_file(Path::new(input_path))
                .expect("failed to load file");

            compiler.process_js_file(fm, handler, options)
        })
    });

    match output {
        Ok(output) => {
            if !output.code.is_empty() {
                write_transpiled_to_file(&output_path, &output.code, "js");
            }

            if output.map.is_some() {
                write_transpiled_to_file(&output_path, &output.map.unwrap(), "js.map");
            }
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn compile_directory(
    input_dir: &str,
    out_dir: &str,
    compiler: &swc::Compiler,
    options: &Options,
    glob_set: &GlobSet,
) {
    for entry in WalkDir::new(input_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && (path
                .extension()
                .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js"))
        {
            compile_file(path, Path::new(out_dir), compiler, options, glob_set);
        }
    }
}

pub fn transpile(
    mut input_path: &str,
    out_dir: &str,
    config_path: &str,
    fallback_legacy_dts: bool,
    minify_output: bool,
) {
    input_path = input_path.trim_start_matches("./");

    let cm = Arc::<SourceMap>::default();
    let compiler = swc::Compiler::new(cm.clone());

    match fetch_tsconfig(config_path) {
        Ok(ts_config) => {
            let config = convert(&ts_config, Some(minify_output), Some(!fallback_legacy_dts));
            let options: Options = Options {
                config,
                ..Default::default()
            };

            // Build a glob set based on the tsconfig exclude
            let mut builder = GlobSetBuilder::new();
            if ts_config.exclude.is_some() {
                let exclude = ts_config.exclude.as_ref().unwrap();
                for e in exclude {
                    builder.add(Glob::new(e).unwrap());
                }
            }

            let glob_set = builder.build().expect("Failed to build glob set");
            let path = Path::new(input_path);

            if path.is_file() {
                return compile_file(path, Path::new(out_dir), &compiler, &options, &glob_set);
            }

            compile_directory(input_path, out_dir, &compiler, &options, &glob_set)
        }
        Err(e) => {
            eprintln!("{}", e)
        }
    }
}
