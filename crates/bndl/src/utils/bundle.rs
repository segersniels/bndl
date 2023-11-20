use log::debug;
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

pub fn bundle(out_path: &Path) -> Result<(), String> {
    let app_dir = env::current_dir().unwrap_or(PathBuf::from("."));
    let dependencies: std::collections::HashMap<String, std::path::PathBuf> =
        bndl_deps::fetch_used_dependencies();

    for (name, path) in dependencies.iter() {
        let compiled_dependency_path = Path::new(path).join(out_path);
        let destination = app_dir.join(out_path).join("node_modules").join(name);

        // Check if we have to copy over the compiled dependency or the source code directly
        let source: std::path::PathBuf = if compiled_dependency_path.exists() {
            compiled_dependency_path
        } else {
            path.to_owned()
        };

        debug!("Copying {:?} to {:?}", source, destination);

        copy_dir_all(&source, &destination)
            .expect(format!("Unable to copy {:?} to {:?}", source, destination).as_str());
    }

    Ok(())
}
