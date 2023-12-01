use bndl_convert::{GlobSetConfig, SerializableOptions, TsConfigJson};
use log::debug;
use notify::{self, RecursiveMode, Watcher};
use rayon::prelude::*;
use std::path::PathBuf;
use std::{env, fs, process};
use std::{path::Path, sync::Arc};
use swc;
use swc_common::{SourceMap, GLOBALS};
use walkdir::{DirEntry, WalkDir};

use crate::utils::{bundle, sourcemap};

/// Removes the output directory if it exists
pub fn clean_out_dir(out_path: &Path) {
    if out_path.as_os_str().is_empty() {
        return;
    }

    let dir_to_delete = env::current_dir()
        .unwrap_or(PathBuf::from("."))
        .join(out_path);

    if dir_to_delete.exists() {
        debug!("Cleaning output directory: {:?}", dir_to_delete);
        fs::remove_dir_all(&dir_to_delete)
            .unwrap_or_else(|_| panic!("Failed to remove directory {:?}", dir_to_delete));
    }
}

/// Creates .d.ts files for the project
fn create_tsc_dts(project: &Path, out_path: &Path) -> std::process::Output {
    let args = vec![
        "tsc",
        "-d",
        "--emitDeclarationOnly",
        "--outDir",
        out_path.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ];

    std::process::Command::new("npx")
        .args(args)
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .expect("Failed to execute command")
}

fn check_to_ignore_dir(entry: &DirEntry, glob_sets: &GlobSetConfig) -> bool {
    glob_sets.exclude.is_match(entry.path()) || entry.file_name() == "node_modules"
}

fn check_to_ignore_file(file: &Path, glob_sets: &GlobSetConfig) -> bool {
    glob_sets.exclude.is_match(file)
        || (!glob_sets.include.is_empty() && !glob_sets.include.is_match(file))
        || file
            .extension()
            .map_or(false, |ext| ext != "ts" && ext != "tsx" && ext != "js")
}

/// Ensures that the source map has the correct source file name and source root

fn compile_file(
    input_path: &Path,
    compiler: &swc::Compiler,
    options: &swc::config::Options,
    glob_sets: &GlobSetConfig,
) {
    // Check if we should ignore the file based on the tsconfig exclude
    // We need to do this because the swc `exclude` is odd and doesn't work as expected
    if check_to_ignore_file(input_path, glob_sets) {
        return;
    }

    let output_path = options.output_path.as_ref().unwrap();
    let output_file_path = output_path.join(input_path).with_extension("js");
    let source_map_path = output_file_path.with_extension("js.map");

    // Create missing directories if they don't exist yet
    if let Some(path) = output_file_path.parent() {
        fs::create_dir_all(path)
            .unwrap_or_else(|_| panic!("Failed to create directory {:?}", path));
    };

    let extended_options = swc::config::Options {
        source_file_name: sourcemap::determine_source_file_name(
            input_path,
            output_file_path.parent().unwrap(),
        ),
        ..options.clone()
    };

    let transform_output = GLOBALS.set(&Default::default(), || {
        swc::try_with_handler(compiler.cm.clone(), Default::default(), |handler| {
            let fm: Arc<swc_common::SourceFile> = compiler
                .cm
                .load_file(input_path)
                .unwrap_or_else(|_| panic!("failed to load file {:?}", input_path));

            compiler.process_js_file(fm, handler, &extended_options)
        })
    });

    match transform_output {
        Ok(mut output) => {
            let source_file_name = &extended_options.source_file_name;
            let source_root = &extended_options.source_root;

            // Extend the source map so it actually has content
            let source_map = output.map.as_ref().map(|source_map| {
                sourcemap::extend_source_map(source_map.to_owned(), source_file_name, source_root)
            });

            if output.code.is_empty() {
                return;
            }

            if let Some(ref source_map) = source_map {
                output.code.push_str("\n//# sourceMappingURL=");
                output
                    .code
                    .push_str(&source_map_path.file_name().unwrap().to_string_lossy());

                fs::write(&source_map_path, source_map)
                    .unwrap_or_else(|_| panic!("Failed to write to {:?}", source_map_path));
            }

            fs::write(&output_file_path, &output.code)
                .unwrap_or_else(|_| panic!("Failed to write to {:?}", output_file_path));
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}

/// Mimic `tsc` behavior by copying over JSON files that are explicitly in the
/// `include` when also `resolveJsonModule` is specified
fn handle_json_file(
    path: &Path,
    options: &swc::config::Options,
    glob_sets: &GlobSetConfig,
    tsconfig: &TsConfigJson,
) {
    if !tsconfig
        .clone()
        .compilerOptions
        .unwrap_or_default()
        .resolveJsonModule
        .unwrap_or_default()
        || !glob_sets.include.is_match(path)
    {
        return;
    }

    let output_path = options.output_path.as_ref().unwrap();
    let output_file_path = output_path.join(path);

    if let Some(path) = output_file_path.parent() {
        fs::create_dir_all(path)
            .unwrap_or_else(|_| panic!("Failed to create directory {:?}", path));
    };

    fs::copy(path, &output_file_path)
        .unwrap_or_else(|_| panic!("Failed to copy JSON to {:?}", output_file_path));
}

fn compile_directory(
    input_path: &Path,
    compiler: &swc::Compiler,
    options: &swc::config::Options,
    glob_sets: &GlobSetConfig,
    tsconfig: &TsConfigJson,
) {
    let mut paths = Vec::new();
    let mut it = WalkDir::new(input_path).into_iter();

    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => panic!("ERROR: {}", err),
            Some(Ok(entry)) => entry,
        };

        let path = entry.path();
        if path.is_dir() && check_to_ignore_dir(&entry, glob_sets) {
            it.skip_current_dir();
            continue;
        } else if path.is_symlink() {
            // Don't bother following symlinks
            continue;
        } else if path
            .extension()
            .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js")
        {
            // Keep track of this path for compiling
            paths.push(path.to_path_buf());
        } else if path.extension().unwrap_or_default() == "json" {
            // Handle JSON files separately
            handle_json_file(path, options, glob_sets, tsconfig);
        }
    }

    // Compile all the files we found in parallel
    paths
        .par_iter()
        .for_each(|path| compile_file(&path, compiler, options, glob_sets));
}

#[derive(Clone)]
pub struct TranspileOptions {
    pub input_path: PathBuf,
    pub out_dir: PathBuf,
    pub config_path: PathBuf,
    pub minify_output: bool,
    pub clean: bool,
    pub bundle: bool,
}

fn prepare_input_path(input_path: &Path) -> PathBuf {
    let mut input_path: PathBuf = input_path.to_path_buf();
    let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));

    // Remove the app directory from the input path and treat it as a relative path
    if input_path.starts_with(&app_dir) {
        input_path = input_path.strip_prefix(&app_dir).unwrap().to_path_buf();
    }

    // Remove the leading "./" if it exists, required for SWC to work
    if input_path.starts_with("./") && input_path != Path::new(".") {
        input_path = input_path.strip_prefix("./").unwrap().to_path_buf();
    }

    input_path
}

pub fn transpile(opts: TranspileOptions, tsconfig: &TsConfigJson) -> Result<(), String> {
    let input_path = prepare_input_path(&opts.input_path);

    if opts.clean {
        clean_out_dir(&opts.out_dir);
    }

    let options = swc::config::Options {
        output_path: Some(opts.out_dir.clone()),
        swcrc: false,
        ..bndl_convert::convert_from_tsconfig(tsconfig, Some(opts.minify_output), None)
    };

    debug!(
        "Options: {}",
        serde_json::to_string_pretty(&SerializableOptions::from(&options)).unwrap()
    );

    // Build glob sets based on the tsconfig include & exclude
    let glob_sets = bndl_convert::determine_include_and_exclude(tsconfig);

    // Prepare SWC compiler
    let cm: Arc<SourceMap> = Arc::<SourceMap>::default();
    let compiler = swc::Compiler::new(cm);

    if input_path.is_file() && input_path.exists() {
        compile_file(&input_path, &compiler, &options, &glob_sets);
    } else {
        compile_directory(&input_path, &compiler, &options, &glob_sets, tsconfig);
    }

    // Rely on `tsc` to provide .d.ts files since SWC's implementation is a bit weird
    if let Some(compiler_options) = tsconfig.clone().compilerOptions {
        if compiler_options.declaration.unwrap_or_default() {
            // Give preference to specified declaration directory in tsconfig
            let declaration_dir = if compiler_options.declarationDir.is_some() {
                Path::new(compiler_options.declarationDir.as_ref().unwrap())
            } else {
                opts.out_dir.as_path()
            };

            create_tsc_dts(&opts.config_path, declaration_dir);
        }
    }

    // Bundle the monorepo dependencies if the flag is set
    if opts.bundle {
        bundle::bundle(&opts.out_dir)?;
    }

    Ok(())
}

fn check_to_ignore_watch_event(event: &notify::Event) -> bool {
    if !event.kind.is_modify() && event.kind.is_create() {
        return true;
    }

    return false;
}

pub fn watch(opts: TranspileOptions, tsconfig: TsConfigJson) -> notify::Result<()> {
    let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));

    // Transpile fully once before we start watching
    if let Err(err) = transpile(opts.clone(), &tsconfig) {
        eprintln!("{err}");
        process::exit(1);
    }

    let input_path = opts.input_path.clone();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                debug!("Incoming event: {:#?}", event);

                // Only recompile if the file was modified or created
                if check_to_ignore_watch_event(&event) {
                    return;
                }

                for mut path in event.paths {
                    path = path.canonicalize().unwrap_or(path);

                    // Ignore files that are in the output directory
                    if path.starts_with(&app_dir)
                        && path
                            .strip_prefix(&app_dir)
                            .unwrap()
                            .starts_with(&opts.out_dir)
                    {
                        debug!("Ignoring path: {:#?}", path);
                        continue;
                    }

                    debug!("File changed: {:?}", path);
                    if let Err(err) = transpile(
                        TranspileOptions {
                            input_path: path,
                            out_dir: opts.out_dir.clone(),
                            config_path: opts.config_path.clone(),
                            minify_output: opts.minify_output,
                            clean: false,
                            bundle: false,
                        },
                        &tsconfig,
                    ) {
                        // Just print the error but keep watching so the user can correct his error
                        eprintln!("{err}");
                    }
                }
            }
            Err(err) => {
                eprintln!("{:?}", err);
            }
        })?;

    watcher.watch(&input_path, RecursiveMode::Recursive)?;

    // The watcher will run asynchronously, so we need to keep the main thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
