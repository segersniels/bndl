use log::debug;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io};

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_symlink() {
            continue;
        } else if file_type.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }

    Ok(())
}

pub fn bundle(app_out_path: &PathBuf) -> Result<(), String> {
    let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));
    let dependencies: std::collections::HashMap<String, std::path::PathBuf> =
        bndl_deps::fetch_used_dependencies();

    dependencies.into_par_iter().for_each(|(name, path)| {
        let config_path = path.join("tsconfig.json");
        let destination = app_dir.join(app_out_path).join("node_modules").join(name);

        let source = match bndl_convert::fetch_tsconfig(&config_path) {
            Ok(ref tsconfig) => {
                // Don't assume all internal dependencies use the same output directory so we have to
                // check the tsconfig.json of each dependency
                let out_dir = bndl_convert::determine_out_dir(tsconfig, None);
                let compiled_dependency_path = path.join(out_dir);

                // Check if we have to copy over the compiled dependency or the source code directly
                if compiled_dependency_path.exists() {
                    compiled_dependency_path
                } else {
                    path.to_owned()
                }
            }
            Err(err) => {
                debug!("{err} for {:#?}", path);
                path.to_owned()
            }
        };

        match copy_dir_all(&source, &destination) {
            Ok(_) => {
                debug!("Copied {:?} to {:?}", source, destination);
            }
            Err(err) => {
                debug!("Unable to copy {:?} to {:?}", source, destination);
                debug!("{err}");
            }
        }
    });

    Ok(())
}
