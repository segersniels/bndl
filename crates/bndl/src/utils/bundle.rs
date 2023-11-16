use log::debug;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io};
use walkdir::{DirEntry, WalkDir};

use super::config::{fetch_package_json, PackageJson};

fn find_workspace_root() -> Result<PathBuf, String> {
    let mut current_dir = env::current_dir().unwrap();

    loop {
        let package_json_path = current_dir.join("package.json");
        if package_json_path.exists() {
            let package_json = fetch_package_json(&package_json_path);
            if package_json.workspaces.is_some() {
                return Ok(current_dir);
            }
        }

        if current_dir.parent().is_none() {
            break;
        }

        current_dir = current_dir.parent().unwrap().to_path_buf();
    }

    Err("Unable to find workspace root".to_string())
}

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

/// Function to check if a directory entry is a 'node_modules' directory
/// and skip it. We aren't interested in any of these contents and are only
/// interested in the internal packages of the monorepo
fn is_node_modules(entry: &DirEntry) -> bool {
    entry.file_type().is_dir() && entry.file_name() == "node_modules"
}

fn fetch_internal_packages(root: &Path) -> HashMap<String, PathBuf> {
    let mut packages = HashMap::new();
    let walker = WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !is_node_modules(e))
        .filter_map(|e| e.ok());

    for entry in walker {
        let path = entry.path();

        // Only interested in package.json files
        if path.is_dir() || path.file_name().unwrap_or_default() != "package.json" {
            continue;
        }

        let package_json = fetch_package_json(path);
        packages.insert(package_json.name, path.parent().unwrap().to_owned());
    }

    debug!("Found internal packages: {:?}", packages);

    packages
}

fn determine_internal_dependencies(
    package_json: &PackageJson,
    root: &Path,
) -> HashMap<String, PathBuf> {
    let packages = fetch_internal_packages(root);
    let dependencies = package_json.clone().dependencies.unwrap_or_default();

    packages
        .iter()
        .filter(|(name, _)| dependencies.contains_key(*name))
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect()
}

pub fn bundle(out_path: &Path) {
    let package_json_path = Path::new("package.json");
    let app_dir = package_json_path.parent().unwrap();
    let package_json = fetch_package_json(package_json_path);

    match find_workspace_root() {
        Ok(root) => {
            let dependencies = determine_internal_dependencies(&package_json, &root);

            debug!("Used internal dependencies: {:?}", dependencies);

            for (name, path) in dependencies.iter() {
                let compiled_dependency_path = Path::new(path).join(out_path);
                let destination = app_dir.join(out_path).join("node_modules").join(name);

                // Check if we have to copy over the compiled dependency or the source code directly
                let source = if compiled_dependency_path.exists() {
                    compiled_dependency_path
                } else {
                    path.to_owned()
                };

                debug!("Copying {:?} to {:?}", source, destination);

                copy_dir_all(source, destination).expect("Unable to copy");
            }
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}
