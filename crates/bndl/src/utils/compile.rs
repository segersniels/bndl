use bndl_convert::SerializableOptions;
use globset::{Glob, GlobSet, GlobSetBuilder};
use log::debug;
use std::path::PathBuf;
use std::{env, fs, process};
use std::{path::Path, sync::Arc};
use swc;
use swc_common::{SourceMap, GLOBALS};
use walkdir::WalkDir;

use crate::utils::bundle;

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
            .expect(format!("Failed to remove directory {:?}", dir_to_delete).as_str());
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

fn check_to_ignore(filename: &Path, glob_set: &GlobSet) -> bool {
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

    if let Some(root) = source_root {
        source_map.set_source_root(Some(root.to_string()));
    }

    let mut buf = vec![];
    source_map
        .to_writer(&mut buf)
        .expect("failed to decode source map");

    buf
}

fn determine_source_file_name(input_path: &Path, output_path: &Path) -> Option<String> {
    pathdiff::diff_paths(
        input_path.canonicalize().unwrap(),
        output_path.canonicalize().unwrap(),
    )
    .map(|diff| diff.to_string_lossy().to_string())
}

fn compile_file(
    input_path: &Path,
    compiler: &swc::Compiler,
    options: &swc::config::Options,
    glob_set: &GlobSet,
) {
    // Check if we should ignore the file based on the tsconfig exclude
    // We need to do this because the swc `exclude` is odd and doesn't work as expected
    if check_to_ignore(input_path, glob_set) {
        return;
    }

    let output_path = options.output_path.as_ref().unwrap();
    let output_file_path = output_path.join(input_path).with_extension("js");
    let source_map_path = output_file_path.with_extension("js.map");

    // Create missing directories if they don't exist yet
    if let Some(path) = output_file_path.parent() {
        fs::create_dir_all(path).expect(format!("Failed to create directory {:?}", path).as_str());
    };

    let transform_output = GLOBALS.set(&Default::default(), || {
        swc::try_with_handler(compiler.cm.clone(), Default::default(), |handler| {
            let fm: Arc<swc_common::SourceFile> = compiler
                .cm
                .load_file(input_path)
                .expect(format!("failed to load file {:?}", input_path).as_str());

            let source_file_name =
                determine_source_file_name(input_path, output_file_path.parent().unwrap());

            compiler.process_js_file(
                fm,
                handler,
                &swc::config::Options {
                    source_file_name,
                    ..options.clone()
                },
            )
        })
    });

    match transform_output {
        Ok(mut output) => {
            let source_file_name = &options.source_file_name;
            let source_root = &options.source_root;

            // Extend the source map so it actually has content
            let source_map = output.map.as_ref().map(|source_map| {
                extend_source_map(source_map.to_owned(), source_file_name, source_root)
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
                    .expect(format!("Failed to write to {:?}", source_map_path).as_str());
            }

            fs::write(&output_file_path, &output.code)
                .expect(format!("Failed to write to {:?}", output_file_path).as_str());
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
    options: &swc::config::Options,
    glob_set: &GlobSet,
) {
    let mut it = WalkDir::new(input_path).into_iter();

    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(err)) => panic!("ERROR: {}", err),
            Some(Ok(entry)) => entry,
        };

        let path = entry.path();
        if path.is_dir() && check_to_ignore(path, glob_set) {
            it.skip_current_dir();
            continue;
        } else if path.is_symlink() {
            // Don't bother following symlinks
            continue;
        } else if path
            .extension()
            .map_or(false, |ext| ext == "ts" || ext == "tsx" || ext == "js")
        {
            compile_file(path, compiler, options, glob_set);
        }
    }
}

#[derive(Clone)]
pub struct TranspileOptions {
    pub filename: PathBuf,
    /// Overrides the internal tsconfig outDir
    pub out_dir: Option<String>,
    pub config_path: PathBuf,
    pub minify_output: bool,
    pub clean: bool,
    pub bundle: bool,
}

pub fn transpile(opts: TranspileOptions) -> Result<(), String> {
    let input_path = Path::new(opts.filename.to_str().unwrap().trim_start_matches("./"));
    let tsconfig = bndl_convert::fetch_tsconfig(&opts.config_path)?;

    let out_dir = if let Some(out_dir) = opts.out_dir {
        PathBuf::from(out_dir)
    } else if let Some(compiler_options) = tsconfig.clone().compilerOptions {
        PathBuf::from(&compiler_options.outDir.unwrap_or(String::from("dist")))
    } else {
        PathBuf::from("dist")
    };

    if opts.clean {
        clean_out_dir(&out_dir);
    }

    let options = swc::config::Options {
        output_path: Some(out_dir.clone()),
        swcrc: false,
        ..bndl_convert::convert_from_tsconfig(&tsconfig, Some(opts.minify_output), None)
    };

    debug!(
        "Options: {}",
        serde_json::to_string_pretty(&SerializableOptions::from(&options)).unwrap()
    );

    // Build a glob set based on the tsconfig exclude
    let mut builder = GlobSetBuilder::new();
    if tsconfig.exclude.is_some() {
        let exclude = tsconfig.exclude.as_ref().unwrap();
        for e in exclude {
            let mut glob = e.to_owned();

            if glob.ends_with('/') {
                glob = glob[0..glob.len() - 1].to_string();
            }

            // Absolute paths can't be matched so ensure we hit all references through a general glob
            if !glob.starts_with("./") && !glob.starts_with('*') {
                glob = format!("*/{glob}/**");
            }

            debug!("Adding {glob} to globset");

            builder.add(Glob::new(glob.as_str()).unwrap());
        }
    }

    let cm = Arc::<SourceMap>::default();
    let compiler = swc::Compiler::new(cm);
    let glob_set = builder.build().expect("Failed to build glob set");

    if input_path.is_file() {
        compile_file(input_path, &compiler, &options, &glob_set);
    } else {
        compile_directory(input_path, &compiler, &options, &glob_set);
    }

    // Rely on `tsc` to provide .d.ts files since SWC's implementation is a bit weird
    if let Some(compiler_options) = tsconfig.clone().compilerOptions {
        if compiler_options.declaration.unwrap_or_default() {
            // Give preference to specified declaration directory in tsconfig
            let declaration_dir = if compiler_options.declarationDir.is_some() {
                Path::new(compiler_options.declarationDir.as_ref().unwrap())
            } else {
                out_dir.as_path()
            };

            create_tsc_dts(&opts.config_path, declaration_dir);
        }
    }

    // Bundle the monorepo dependencies if the flag is set
    if opts.bundle {
        match bundle::bundle(&out_dir) {
            Ok(_) => {
                debug!("Successfully bundled all dependencies");
            }
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    }

    Ok(())
}
