use bndl_convert::{convert, TsConfigJson};
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::debug;
use std::{env, fs, process};
use std::{path::Path, sync::Arc};
use swc::{self, config::Options};
use swc_common::{SourceMap, GLOBALS};
use walkdir::WalkDir;

/// Removes the output directory if it exists
pub fn clean_out_dir(out_path: &Path) {
    let dir_to_delete = env::current_dir()
        .unwrap_or(Path::new(".").to_path_buf())
        .join(out_path);

    if dir_to_delete.exists() {
        debug!("Cleaning output directory: {:?}", dir_to_delete);
        fs::remove_dir_all(dir_to_delete).expect("Failed to remove directory");
    }
}

/// Creates .d.ts files for the project
fn create_tsc_dts(project: &str, out_path: &Path) -> std::process::Output {
    let args = vec![
        "tsc",
        "-d",
        "--emitDeclarationOnly",
        "--outDir",
        out_path.to_str().unwrap(),
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
    out_path: &Path,
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
    let output_path = out_path.join(input_path);
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
    input_path: &Path,
    out_path: &Path,
    compiler: &swc::Compiler,
    options: &Options,
    glob_set: &GlobSet,
) {
    for entry in WalkDir::new(input_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file()
            && (path
                .extension()
                .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js"))
        {
            compile_file(path, Path::new(out_path), compiler, options, glob_set);
        }
    }
}

pub fn transpile(
    mut input_path: &Path,
    out_path: &Path,
    ts_config: &TsConfigJson,
    config_path: &str,
    minify_output: bool,
) {
    input_path = Path::new(input_path.to_str().unwrap().trim_start_matches("./"));

    let cm = Arc::<SourceMap>::default();
    let compiler = swc::Compiler::new(cm.clone());

    let config = convert(ts_config, Some(minify_output), None);
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

    if input_path.is_file() {
        compile_file(input_path, out_path, &compiler, &options, &glob_set);
    } else {
        compile_directory(input_path, out_path, &compiler, &options, &glob_set);
    }

    // Rely on `tsc` to provide .d.ts files since SWC's implementation is a bit weird
    if ts_config.compilerOptions.declaration.unwrap_or_default() {
        // Give preference to specified declaration directory in tsconfig
        let declaration_dir = if ts_config.compilerOptions.declarationDir.is_some() {
            Path::new(ts_config.compilerOptions.declarationDir.as_ref().unwrap())
        } else {
            out_path
        };

        create_tsc_dts(config_path, declaration_dir);
    }
}
