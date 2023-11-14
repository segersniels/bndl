use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::{env, io};
use walkdir::WalkDir;

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

fn fetch_internal_packages(packages_dir: &Path) -> HashMap<String, PathBuf> {
    let mut packages = HashMap::new();

    for entry in WalkDir::new(packages_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() {
            continue;
        }

        let package_json_path = path.join("package.json");
        let package_json = fetch_package_json(package_json_path.as_path());
        packages.insert(package_json.name, path.to_owned());
    }

    packages
}

fn determine_internal_dependencies(
    package_json: &PackageJson,
    packages_dir: &Path,
) -> HashMap<String, PathBuf> {
    let packages = fetch_internal_packages(packages_dir);
    let dependencies = package_json.clone().dependencies.unwrap_or_default();

    packages
        .iter()
        .filter(|(name, _)| dependencies.contains_key(*name))
        .map(|(name, path)| (name.clone(), path.clone()))
        .collect()
}

pub fn bundle(out_dir: &str, packages_dir: &str) {
    let package_json_path = Path::new("package.json");
    let app_dir = package_json_path.parent().unwrap();
    let package_json = fetch_package_json(package_json_path);

    let root = find_workspace_root().unwrap();
    let packages_dir = root.join(packages_dir);
    let dependencies = determine_internal_dependencies(&package_json, &packages_dir);

    for (name, path) in dependencies.iter() {
        let compiled_dependency_path = Path::new(path).join(out_dir);
        let destination = app_dir.join(out_dir).join("node_modules").join(name);

        // Check if we have to copy over the compiled dependency or the source code directly
        let source = if compiled_dependency_path.exists() {
            compiled_dependency_path
        } else {
            path.to_owned()
        };

        copy_dir_all(source, destination).expect("Unable to copy");
    }
}
