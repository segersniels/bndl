use log::debug;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io};

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
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
        let source = if let Ok(ref tsconfig) = bndl_convert::fetch_tsconfig(&config_path) {
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
        } else {
            path.to_owned()
        };

        debug!("Copying {:?} to {:?}", source, destination);

        copy_dir_all(&source, &destination)
            .unwrap_or_else(|_| panic!("Unable to copy {:?} to {:?}", source, destination));
    });

    Ok(())
}
