use bndl_convert::{convert, SerializableOptions, TsConfigJson};
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

fn check_to_ignore_file(filename: &Path, glob_set: &GlobSet) -> bool {
    glob_set.is_match(filename)
}

/// Ensures that the source map has the correct source file name and source root
fn extend_source_map(
    source_map: String,
    source_file_name: &Option<String>,
    source_root: &Option<String>,
) -> Vec<u8> {
    let mut source_map = swc::sourcemap::SourceMap::from_reader(source_map.as_bytes())
        .expect("failed to encode source map");

    if !source_map.get_token_count() != 0 {
        if let Some(ref source_file_name) = source_file_name {
            source_map.set_source(0u32, source_file_name);
        }
    }

    if source_root.is_some() {
        source_map.set_source_root(source_root.clone());
    }

    let mut buf = vec![];
    source_map
        .to_writer(&mut buf)
        .expect("failed to decode source map");

    return buf;
}

fn compile_file(
    input_path: &Path,
    compiler: &swc::Compiler,
    options: &Options,
    glob_set: &GlobSet,
) {
    // Check if we should ignore the file based on the tsconfig exclude
    // We need to do this because the swc `exclude`` is odd and doesn't work as expected
    if check_to_ignore_file(input_path, glob_set) {
        return;
    }

    let transform_output = GLOBALS.set(&Default::default(), || {
        swc::try_with_handler(compiler.cm.clone(), Default::default(), |handler| {
            let fm: Arc<swc_common::SourceFile> = compiler
                .cm
                .load_file(input_path)
                .expect("failed to load file");

            compiler.process_js_file(fm, handler, &options)
        })
    });

    match transform_output {
        Ok(mut output) => {
            let output_path = options.output_path.as_ref().unwrap();
            let source_file_name = &options.source_file_name;
            let source_root = &options.source_root;

            // Extend the source map so it actually has content
            let source_map = if let Some(ref source_map) = &output.map {
                Some(extend_source_map(
                    source_map.to_owned(),
                    source_file_name,
                    source_root,
                ))
            } else {
                None
            };

            if output.code.is_empty() {
                return;
            }

            let output_file_path = output_path.join(input_path).with_extension("js");
            if let Some(p) = output_file_path.parent() {
                fs::create_dir_all(p).expect("Failed to create directory");
            };

            if let Some(ref source_map) = source_map {
                let source_map_path = output_file_path.with_extension("js.map");

                output.code.push_str("\n//# sourceMappingURL=");
                output
                    .code
                    .push_str(&source_map_path.file_name().unwrap().to_string_lossy());

                fs::write(source_map_path, source_map).expect("Failed to write to file");
            }

            fs::write(output_file_path, &output.code).expect("Failed to write to file");
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn compile_directory(
    input_path: &Path,
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
            compile_file(path, compiler, options, glob_set);
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
    let output_path = env::current_dir()
        .unwrap_or(Path::new(".").to_path_buf())
        .join(out_path);

    let cm = Arc::<SourceMap>::default();
    let compiler = swc::Compiler::new(cm);
    let options = swc::config::Options {
        output_path: Some(output_path.clone()),
        swcrc: false,
        ..convert(ts_config, Some(minify_output), None)
    };

    debug!(
        "Options: {}",
        serde_json::to_string_pretty(&SerializableOptions::from(&options)).unwrap()
    );

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
        compile_file(input_path, &compiler, &options, &glob_set);
    } else {
        compile_directory(input_path, &compiler, &options, &glob_set);
    }

    // Rely on `tsc` to provide .d.ts files since SWC's implementation is a bit weird
    if ts_config.compilerOptions.declaration.unwrap_or_default() {
        // Give preference to specified declaration directory in tsconfig
        let declaration_dir = if ts_config.compilerOptions.declarationDir.is_some() {
            Path::new(ts_config.compilerOptions.declarationDir.as_ref().unwrap())
        } else {
            output_path.as_path()
        };

        create_tsc_dts(config_path, declaration_dir);
    }
}
